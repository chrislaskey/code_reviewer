//! Memberships: a user in a community, with a role the app interprets,
//! `member` for every row today. Joining writes the row; leaving
//! soft-deletes it and takes the member's own rows in the community
//! along, every row under it deleted at the one moment; rejoining
//! revives the same row and exactly what left with it, see
//! `SyncedTable::delete_with_membership`. Once the purge has removed the
//! row, rejoining starts fresh.
//!
//! The server decides these rows: a device's event is what the person
//! meant, applied through the same join and leave, and the server's row
//! comes back whatever the device wrote.

use crate::data::app_communities;
use crate::data::app_users_communities::{self, Membership, ROLE_MEMBER, TABLE};
use crate::data::trial_users;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use thiserror::Error;
use uuid::Uuid;

use crate::app::{communities, users};
use crate::data::app_communities::Community;
use crate::data::app_events::{self, Conflicts, Event, EventKind, Fields};
use crate::data::app_sync_backfills;
use crate::data::app_users::User;
use crate::database::Transaction;
use crate::deployment::{Deployment, Role, now};
use crate::helpers::json;
use crate::sync::TableRow;
use crate::sync::VersionRange;
use crate::sync::events::{self, Answer, AnswerStatus};
use crate::sync::synced_table::{LocalCreate, SyncedTable, TableError};
use crate::sync::tables;

const CREATE_FIELDS: &[&str] = &["user_id", "community_id", "role"];
const UPDATE_FIELDS: &[&str] = &["deleted_at"];

#[derive(Debug, Error)]
pub enum MembershipError {
    #[error("not a member of community {community_id}")]
    NotFound { community_id: Uuid },
    #[error("community {id} not found")]
    UnknownCommunity { id: Uuid },
    #[error("{0}")]
    Invalid(String),
    #[error("the change was refused: {0}")]
    Rejected(String),
    #[error(transparent)]
    Record(#[from] crate::sync::events::RecordError),
    /// A synced table's own module failed.
    #[error(transparent)]
    Table(#[from] TableError),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl From<serde_json::Error> for MembershipError {
    fn from(error: serde_json::Error) -> MembershipError {
        MembershipError::Invalid(error.to_string())
    }
}

/// The user's live memberships, oldest first.
pub async fn list_mine(
    deployment: &Deployment,
    user: &User,
) -> Result<Vec<Membership>, MembershipError> {
    let mut tx = deployment.begin().await?;
    let memberships =
        app_users_communities::list(&mut tx, app_users_communities::Filter::User(user.id)).await?;
    tx.commit().await?;
    Ok(memberships)
}

/// Joins the community, or rejoins it, reviving the membership left
/// behind and what left with it. The live membership either way.
pub async fn create(
    deployment: &Deployment,
    user: &User,
    community_id: Uuid,
) -> Result<Membership, MembershipError> {
    let mut tx = deployment.begin().await?;
    let membership = create_in(&mut tx, deployment.role, user, community_id).await?;
    tx.commit().await?;
    Ok(membership)
}

/// Leaves the community, taking the user's own posts there along.
pub async fn delete(
    deployment: &Deployment,
    user: &User,
    community_id: Uuid,
) -> Result<(), MembershipError> {
    let mut tx = deployment.begin().await?;
    delete_in(&mut tx, deployment.role, user, community_id).await?;
    tx.commit().await?;
    Ok(())
}

/// Applies an event of the user's to their membership and answers it.
pub async fn apply_event(
    deployment: &Deployment,
    user: &User,
    event: &Event,
) -> Result<Answer, MembershipError> {
    let mut tx = deployment.begin().await?;
    let applied = apply_in(&mut tx, deployment.role, user, event).await?;
    tx.commit().await?;
    Ok(applied.answer)
}

pub fn table_row(membership: &Membership) -> Result<TableRow, MembershipError> {
    Ok(TableRow {
        table: TABLE.to_string(),
        row: serde_json::to_value(membership)?,
    })
}

pub(crate) async fn create_in(
    tx: &mut Transaction,
    role: Role,
    user: &User,
    community_id: Uuid,
) -> Result<Membership, MembershipError> {
    let event = match app_users_communities::replication::get_by_user_and_community(
        &mut *tx,
        user.id,
        community_id,
    )
    .await?
    {
        None => Event::create(
            TABLE,
            Uuid::now_v7(),
            json::object_fields(
                json!({ "user_id": user.id, "community_id": community_id, "role": ROLE_MEMBER }),
            ),
            now(),
        ),
        Some(membership) if membership.deleted_at.is_none() => return Ok(membership),
        Some(left) => Event::update(
            TABLE,
            left.id,
            left.version,
            json::object_fields(json!({ "deleted_at": left.deleted_at })),
            json::object_fields(json!({ "deleted_at": null })),
            now(),
        ),
    };
    apply_in(tx, role, user, &event).await?.membership()
}

pub(crate) async fn delete_in(
    tx: &mut Transaction,
    role: Role,
    user: &User,
    community_id: Uuid,
) -> Result<Membership, MembershipError> {
    let Some(membership) = get_live_in(tx, user.id, community_id).await? else {
        return Err(MembershipError::NotFound { community_id });
    };
    let event = Event::update(
        TABLE,
        membership.id,
        membership.version,
        json::object_fields(json!({ "deleted_at": null })),
        json::object_fields(json!({ "deleted_at": now() })),
        now(),
    );
    apply_in(tx, role, user, &event).await?.membership()
}

/// The user's live membership in the community, if any.
pub(crate) async fn get_live_in(
    tx: &mut Transaction,
    user_id: Uuid,
    community_id: Uuid,
) -> Result<Option<Membership>, MembershipError> {
    Ok(app_users_communities::get(tx, user_id, community_id).await?)
}

pub(crate) async fn get_in(
    tx: &mut Transaction,
    id: Uuid,
) -> Result<Option<Membership>, MembershipError> {
    Ok(app_users_communities::replication::get(tx, id).await?)
}

/// Writes the server's row over the device's, unless an event of the
/// device's for it is still pending; true when it was written.
pub(crate) async fn apply_server_row(
    tx: &mut Transaction,
    row: &Value,
) -> Result<bool, MembershipError> {
    let membership: Membership = serde_json::from_value(row.clone())?;
    if app_events::has_pending(tx, TABLE, membership.id).await? {
        return Ok(false);
    }
    app_users_communities::replication::upsert(&mut *tx, &membership).await?;
    Ok(true)
}

pub(crate) async fn delete_row(tx: &mut Transaction, id: Uuid) -> Result<(), MembershipError> {
    Ok(app_users_communities::maintenance::delete(tx, id).await?)
}

pub(crate) async fn list_synced_after(
    tx: &mut Transaction,
    user: &User,
    range: &VersionRange,
    limit: usize,
) -> Result<Vec<Membership>, MembershipError> {
    Ok(app_users_communities::replication::list_synced_after(tx, user.id, range, limit).await?)
}

struct Applied {
    answer: Answer,
    membership: Option<Membership>,
}

impl Applied {
    fn membership(self) -> Result<Membership, MembershipError> {
        match (self.answer.status, self.membership) {
            (AnswerStatus::Rejected, _) => Err(MembershipError::Rejected(
                self.answer.reason.unwrap_or_else(|| "rejected".to_string()),
            )),
            (_, Some(membership)) => Ok(membership),
            (_, None) => Err(MembershipError::Invalid("no membership".to_string())),
        }
    }
}

/// Applies the event in the transaction, as what the person meant: a
/// create is a join, an update of `deleted_at` is a leave or a rejoin,
/// whatever the server already holds for the pair. Only the user's own
/// membership, in a community that exists.
async fn apply_in(
    tx: &mut Transaction,
    role: Role,
    user: &User,
    event: &Event,
) -> Result<Applied, MembershipError> {
    if role == Role::Server
        && let Some(recorded) = app_events::get(tx, event.id).await?
    {
        let membership = get_in(tx, event.row_id).await?;
        let server = membership.as_ref().map(table_row).transpose()?;
        return Ok(Applied {
            answer: recorded.answer(server),
            membership,
        });
    }
    // A retry is answered above without waiting for the feed. Two at once
    // both reach here, and the second fails on the event's id.
    crate::data::app_sync_versions::lock_for_role(tx, role).await?;

    let allowed = match event.kind {
        EventKind::Create => CREATE_FIELDS,
        EventKind::Update => UPDATE_FIELDS,
    };
    if let Some(reason) = events::not_editable(&event.changes, allowed) {
        let membership = get_in(tx, event.row_id).await?;
        return rejected(event, reason, membership);
    }

    let membership = match event.kind {
        EventKind::Create => {
            let user_id = id_field(&event.changes, "user_id");
            let community_id = id_field(&event.changes, "community_id");
            if user_id != Some(user.id) {
                return rejected(event, "not_own_membership", None);
            }
            let Some(community_id) = community_id else {
                return rejected(event, "unknown_community", None);
            };
            match app_communities::replication::get(&mut *tx, community_id).await? {
                Some(community) if community.deleted_at.is_none() => {}
                _ => return rejected(event, "unknown_community", None),
            }
            // A trial user and anyone else are never in one community
            if trial_users::join_is_walled_off(&mut *tx, user.id, community_id).await? {
                return rejected(event, "unknown_community", None);
            }
            match app_users_communities::replication::get_by_user_and_community(
                &mut *tx,
                user.id,
                community_id,
            )
            .await?
            {
                Some(existing) if existing.deleted_at.is_none() => existing,
                Some(left) => revive(tx, role, left).await?,
                None => {
                    if let Some(existing) = get_in(tx, event.row_id).await? {
                        return rejected(event, "exists", Some(existing));
                    }
                    let now = now();
                    let (version, last_synced_at) = match role {
                        Role::Server => (1, Some(now)),
                        Role::Device => (0, None),
                    };
                    let membership = Membership {
                        id: event.row_id,
                        user_id: user.id,
                        community_id,
                        role: ROLE_MEMBER.to_string(),
                        inserted_at: event.row_inserted_at(now),
                        updated_at: now,
                        version,
                        last_synced_at,
                        global_sync_version: None,
                        deleted_at: None,
                        deleted_by_id: None,
                    };
                    app_users_communities::create(&mut *tx, role, membership).await?
                }
            }
        }
        EventKind::Update => {
            let Some(membership) = get_in(tx, event.row_id).await? else {
                return rejected(event, "unknown_membership", None);
            };
            if membership.user_id != user.id {
                return rejected(event, "not_own_membership", Some(membership));
            }
            let deleted_at: Option<DateTime<Utc>> = match event.changes.get("deleted_at") {
                Some(value) => match serde_json::from_value(value.clone()) {
                    Ok(deleted_at) => deleted_at,
                    Err(error) => {
                        return rejected(event, format!("invalid: {error}"), Some(membership));
                    }
                },
                None => membership.deleted_at,
            };
            match (membership.deleted_at, deleted_at) {
                (Some(_), None) => {
                    match app_communities::replication::get(&mut *tx, membership.community_id)
                        .await?
                    {
                        Some(community) if community.deleted_at.is_none() => {}
                        _ => return rejected(event, "unknown_community", Some(membership)),
                    }
                    revive(tx, role, membership).await?
                }
                (None, Some(at)) => leave_at(tx, role, user, membership, at).await?,
                _ => membership,
            }
        }
    };
    let answer = Answer::applied(
        event.id,
        membership.version,
        Conflicts::new(),
        table_row(&membership)?,
    );
    match role {
        Role::Server => events::record(tx, user.id, event, &answer).await?,
        Role::Device => events::queue(tx, user.id, event).await?,
    }
    Ok(Applied {
        answer,
        membership: Some(membership),
    })
}

/// Soft-deletes the membership at the moment, and every live row under
/// it at the same moment.
async fn leave_at(
    tx: &mut Transaction,
    role: Role,
    user: &User,
    mut membership: Membership,
    at: DateTime<Utc>,
) -> Result<Membership, MembershipError> {
    membership.deleted_at = Some(at);
    membership.deleted_by_id = Some(user.id);
    write_changed(tx, role, &mut membership).await?;
    delete_rows_with(tx, role, &membership).await?;
    Ok(membership)
}

/// Soft-deletes every live membership in the deleted community at the
/// community's moment, and every live row under them; copied down, not
/// logged, as every delete with a parent is.
pub(crate) async fn delete_with_community(
    tx: &mut Transaction,
    role: Role,
    community: &Community,
) -> Result<(), MembershipError> {
    for mut membership in app_users_communities::list(
        &mut *tx,
        app_users_communities::Filter::Community(community.id),
    )
    .await?
    {
        membership.deleted_at = community.deleted_at;
        membership.deleted_by_id = community.deleted_by_id;
        write_changed(tx, role, &mut membership).await?;
        delete_rows_with(tx, role, &membership).await?;
    }
    Ok(())
}

/// Clears the membership's delete, and the delete of every row that
/// left with it.
async fn revive(
    tx: &mut Transaction,
    role: Role,
    mut membership: Membership,
) -> Result<Membership, MembershipError> {
    let Some(left_at) = membership.deleted_at else {
        return Ok(membership);
    };
    membership.deleted_at = None;
    membership.deleted_by_id = None;
    write_changed(tx, role, &mut membership).await?;
    for table in tables::list() {
        table
            .revive_with_membership(tx, role, &membership, left_at)
            .await?;
    }
    Ok(membership)
}

/// Deletes every table's rows under the membership, children first.
async fn delete_rows_with(
    tx: &mut Transaction,
    role: Role,
    membership: &Membership,
) -> Result<(), MembershipError> {
    for table in tables::list().iter().rev() {
        table.delete_with_membership(tx, role, membership).await?;
    }
    Ok(())
}

/// Writes the membership over its row as the role does: the server
/// increments the version and sets the sync time.
async fn write_changed(
    tx: &mut Transaction,
    role: Role,
    membership: &mut Membership,
) -> Result<(), MembershipError> {
    let now = now();
    membership.updated_at = now;
    if role == Role::Server {
        membership.version += 1;
        membership.last_synced_at = Some(now);
    }
    *membership = app_users_communities::update(&mut *tx, role, membership.clone()).await?;
    Ok(())
}

fn rejected(
    event: &Event,
    reason: impl Into<String>,
    membership: Option<Membership>,
) -> Result<Applied, MembershipError> {
    let server = membership.as_ref().map(table_row).transpose()?;
    Ok(Applied {
        answer: Answer::rejected(event.id, reason, server),
        membership,
    })
}

pub(crate) fn id_field(fields: &Fields, name: &str) -> Option<Uuid> {
    fields
        .get(name)
        .and_then(Value::as_str)
        .and_then(|id| Uuid::parse_str(id).ok())
}

/// Memberships as a synced table, see `synced_tables`.
pub struct Synced;

#[async_trait]
impl SyncedTable for Synced {
    fn foreign_keys(&self) -> &'static [(&'static str, &'static str)] {
        &[
            ("user_id", "app_users"),
            ("community_id", "app_communities"),
        ]
    }

    fn table(&self) -> &'static str {
        TABLE
    }

    fn owner_column(&self) -> Option<&'static str> {
        Some("user_id")
    }

    async fn apply_in(
        &self,
        tx: &mut Transaction,
        role: Role,
        user: &User,
        event: &Event,
    ) -> Result<Answer, TableError> {
        Ok(apply_in(tx, role, user, event).await?.answer)
    }

    async fn list_synced_after(
        &self,
        tx: &mut Transaction,
        user: &User,
        range: &VersionRange,
        limit: usize,
    ) -> Result<Vec<TableRow>, TableError> {
        let memberships = list_synced_after(tx, user, range, limit).await?;
        Ok(memberships
            .iter()
            .map(table_row)
            .collect::<Result<_, _>>()?)
    }

    async fn parents(
        &self,
        tx: &mut Transaction,
        row: &TableRow,
    ) -> Result<Vec<TableRow>, TableError> {
        let membership: Membership = serde_json::from_value(row.row.clone())?;
        let mut parents = Vec::new();
        if let Some(user) = users::get_in(tx, membership.user_id).await? {
            parents.push(users::table_row(&user)?);
        }
        if let Some(community) = communities::get_in(tx, membership.community_id).await? {
            parents.push(communities::table_row(&community)?);
        }
        Ok(parents)
    }

    /// A live membership of the device's user that the device did not
    /// have, or had left, records the community to backfill.
    async fn apply_server_row(
        &self,
        tx: &mut Transaction,
        user: &User,
        row: &TableRow,
    ) -> Result<bool, TableError> {
        let incoming: Membership = serde_json::from_value(row.row.clone())?;
        let joined = incoming.user_id == user.id
            && incoming.deleted_at.is_none()
            && get_in(tx, incoming.id).await?.is_none_or(|had| {
                had.deleted_at.is_some()
                    || had.global_sync_version.is_none()
                    || incoming.version > had.version
            });
        let written = apply_server_row(tx, &row.row).await?;
        if written && joined {
            app_sync_backfills::create(&mut *tx, incoming.community_id).await?;
        }
        Ok(written)
    }

    async fn delete_row(&self, tx: &mut Transaction, id: Uuid) -> Result<(), TableError> {
        Ok(delete_row(tx, id).await?)
    }

    async fn local_user_creates(
        &self,
        tx: &mut Transaction,
        user: &User,
    ) -> Result<Vec<LocalCreate>, TableError> {
        let memberships =
            app_users_communities::list(&mut *tx, app_users_communities::Filter::User(user.id))
                .await?;
        Ok(memberships
            .into_iter()
            .map(|membership| LocalCreate {
                id: membership.id,
                inserted_at: membership.inserted_at,
                parents: vec![membership.community_id],
                changes: json::object_fields(json!({
                    "user_id": user.id,
                    "community_id": membership.community_id,
                    "role": membership.role,
                })),
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{communities, users};
    use crate::database::{Database, with_each_database};
    use crate::sync::test_notes;

    fn server(database: Database) -> Deployment {
        Deployment::new(database, Role::Server)
    }

    async fn note_bodies(deployment: &Deployment, user: &User) -> Vec<String> {
        test_notes::list(deployment, user)
            .await
            .expect("the notes")
            .into_iter()
            .map(|note| note.body)
            .collect()
    }

    #[tokio::test]
    async fn memberships_delete_takes_the_members_rows_along_and_create_brings_back_what_left() {
        with_each_database(|database| async move {
            let deployment = server(database);
            let alice = users::create(&deployment, "alice@example.com", "Alice")
                .await
                .expect("a user");
            let bob = users::create(&deployment, "bob@example.com", "Bob")
                .await
                .expect("a user");
            let gardening = communities::create(&deployment, &alice, "Gardening")
                .await
                .expect("a community");
            create(&deployment, &bob, gardening.id)
                .await
                .expect("joined");
            let roses = test_notes::create(&deployment, &alice, "roses", Some(gardening.id))
                .await
                .expect("a note");
            let weeds = test_notes::create(&deployment, &alice, "weeds", Some(gardening.id))
                .await
                .expect("a note");
            test_notes::create(&deployment, &bob, "bob's", Some(gardening.id))
                .await
                .expect("a note");
            test_notes::update_community(&deployment, &alice, weeds.id, None)
                .await
                .expect("taken out on purpose");
            assert_eq!(note_bodies(&deployment, &bob).await, vec!["bob's", "roses"]);

            let membership = list_mine(&deployment, &alice)
                .await
                .expect("memberships")
                .remove(0);
            delete(&deployment, &alice, gardening.id)
                .await
                .expect("left");
            assert_eq!(note_bodies(&deployment, &bob).await, vec!["bob's"]);
            assert_eq!(
                note_bodies(&deployment, &alice).await,
                vec!["weeds"],
                "roses left with her, weeds was taken out before"
            );
            let mut tx = deployment.begin().await.expect("a transaction");
            let left = get_in(&mut tx, membership.id)
                .await
                .expect("the row")
                .expect("kept");
            let roses = test_notes::get_in(&mut tx, roses.id)
                .await
                .expect("the note")
                .expect("kept");
            assert_eq!(roses.deleted_at, left.deleted_at, "the one moment");
            tx.commit().await.expect("committed");

            let rejoined = create(&deployment, &alice, gardening.id)
                .await
                .expect("rejoined");
            assert_eq!(rejoined.id, membership.id, "the same row");
            assert_eq!(
                note_bodies(&deployment, &bob).await,
                vec!["bob's", "roses"],
                "exactly what left with her came back"
            );
        })
        .await;
    }

    #[tokio::test]
    async fn memberships_create_needs_a_community() {
        with_each_database(|database| async move {
            let deployment = server(database);
            let alice = users::create(&deployment, "alice@example.com", "Alice")
                .await
                .expect("a user");
            let error = create(&deployment, &alice, Uuid::now_v7())
                .await
                .expect_err("no such community");
            assert!(
                matches!(error, MembershipError::Rejected(reason) if reason == "unknown_community")
            );
        })
        .await;
    }

    #[tokio::test]
    async fn memberships_apply_event_refuses_someone_elses_membership() {
        with_each_database(|database| async move {
            let deployment = server(database);
            let alice = users::create(&deployment, "alice@example.com", "Alice")
                .await
                .expect("a user");
            let bob = users::create(&deployment, "bob@example.com", "Bob")
                .await
                .expect("a user");
            let gardening = communities::create(&deployment, &alice, "Gardening")
                .await
                .expect("a community");
            let alices = list_mine(&deployment, &alice)
                .await
                .expect("memberships")
                .remove(0);

            let event = Event::create(
                TABLE,
                Uuid::now_v7(),
                json::object_fields(
                    json!({ "user_id": alice.id, "community_id": gardening.id, "role": "member" }),
                ),
                now(),
            );
            let answer = apply_event(&deployment, &bob, &event)
                .await
                .expect("answered");
            assert_eq!(answer.status, AnswerStatus::Rejected);
            assert_eq!(answer.reason.as_deref(), Some("not_own_membership"));

            let event = Event::update(
                TABLE,
                alices.id,
                alices.version,
                json::object_fields(json!({ "deleted_at": null })),
                json::object_fields(json!({ "deleted_at": now() })),
                now(),
            );
            let answer = apply_event(&deployment, &bob, &event)
                .await
                .expect("answered");
            assert_eq!(answer.status, AnswerStatus::Rejected);
            assert_eq!(answer.reason.as_deref(), Some("not_own_membership"));
            assert_eq!(
                list_mine(&deployment, &alice)
                    .await
                    .expect("still in")
                    .len(),
                1
            );
        })
        .await;
    }
}
