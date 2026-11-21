//! Memberships: a user in a community, with a role the app interprets,
//! `member` for every row today. Joining writes the row; leaving
//! soft-deletes it and takes the member's own posts in the community
//! along, every row under it deleted at the one moment; rejoining
//! revives the same row and exactly what left with it, see
//! `posts_communities`. Once the purge has removed the row, rejoining
//! starts fresh.
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

use crate::app::events::{self, Answer, AnswerStatus};
use crate::app::posts_communities;
use crate::app::sync::TableRow;
use crate::app::synced_table::{LocalCreate, SyncedTable, TableError};
use crate::app::{communities, users};
use crate::data::app_communities::Community;
use crate::data::app_events::{self, Conflicts, Event, EventKind, Fields};
use crate::data::app_sync_backfills;
use crate::data::app_sync_state::Cursor;
use crate::data::app_users::User;
use crate::database::Transaction;
use crate::deployment::{Deployment, Role, now};
use crate::helpers::json;

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
    Record(#[from] crate::app::events::RecordError),
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
pub async fn join(
    deployment: &Deployment,
    user: &User,
    community_id: Uuid,
) -> Result<Membership, MembershipError> {
    let mut tx = deployment.begin().await?;
    let membership = join_in(&mut tx, deployment.role, user, community_id).await?;
    tx.commit().await?;
    Ok(membership)
}

/// Leaves the community, taking the user's own posts there along.
pub async fn leave(
    deployment: &Deployment,
    user: &User,
    community_id: Uuid,
) -> Result<(), MembershipError> {
    let mut tx = deployment.begin().await?;
    leave_in(&mut tx, deployment.role, user, community_id).await?;
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

pub(crate) async fn join_in(
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

pub(crate) async fn leave_in(
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
    after: &Cursor,
    limit: usize,
) -> Result<Vec<Membership>, MembershipError> {
    Ok(app_users_communities::replication::list_synced_after(tx, user.id, after, limit).await?)
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
                        deleted_at: None,
                        deleted_by_id: None,
                    };
                    app_users_communities::create(&mut *tx, &membership).await?;
                    membership
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
    posts_communities::delete_with_membership(tx, role, &membership)
        .await
        .map_err(|error| MembershipError::Invalid(error.to_string()))?;
    Ok(membership)
}

/// Soft-deletes every live membership in the deleted community at the
/// community's moment, and every live row under them; copied down, not
/// logged, as a post's delete is.
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
        posts_communities::delete_with_membership(tx, role, &membership)
            .await
            .map_err(|error| MembershipError::Invalid(error.to_string()))?;
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
    posts_communities::revive_with_membership(tx, role, &membership, left_at)
        .await
        .map_err(|error| MembershipError::Invalid(error.to_string()))?;
    Ok(membership)
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
    app_users_communities::update(&mut *tx, membership).await?;
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
        after: &Cursor,
        limit: usize,
    ) -> Result<Vec<TableRow>, TableError> {
        let memberships = list_synced_after(tx, user, after, limit).await?;
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
            && get_in(tx, incoming.id)
                .await?
                .is_none_or(|had| had.deleted_at.is_some() || had.last_synced_at.is_none());
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
    use crate::app::{communities, posts, posts_communities, users};
    use crate::data::app_posts_communities;
    use crate::database::{Database, with_each_database};

    fn server(database: Database) -> Deployment {
        Deployment::new(database, Role::Server)
    }

    async fn bodies(deployment: &Deployment, user: &User) -> Vec<String> {
        posts::list(deployment, user)
            .await
            .expect("the posts")
            .into_iter()
            .map(|listed| listed.post.body)
            .collect()
    }

    #[tokio::test]
    async fn memberships_leave_takes_own_posts_along_and_rejoin_brings_back_what_left() {
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
            join(&deployment, &bob, gardening.id).await.expect("joined");
            let roses = posts::create(&deployment, &alice, "roses", &[gardening.id])
                .await
                .expect("a post");
            let weeds = posts::create(&deployment, &alice, "weeds", &[gardening.id])
                .await
                .expect("a post");
            let bobs = posts::create(&deployment, &bob, "bob's", &[gardening.id])
                .await
                .expect("a post");
            posts_communities::remove(&deployment, &alice, weeds.id, gardening.id)
                .await
                .expect("removed on purpose");
            assert_eq!(bodies(&deployment, &bob).await, vec!["bob's", "roses"]);

            let membership = list_mine(&deployment, &alice)
                .await
                .expect("memberships")
                .remove(0);
            leave(&deployment, &alice, gardening.id)
                .await
                .expect("left");
            assert!(
                list_mine(&deployment, &alice)
                    .await
                    .expect("none")
                    .is_empty()
            );
            assert_eq!(bodies(&deployment, &bob).await, vec!["bob's"]);
            assert_eq!(
                bodies(&deployment, &alice).await,
                vec!["weeds", "roses"],
                "her own posts stay hers"
            );
            let mut tx = deployment.begin().await.expect("a transaction");
            let left = get_in(&mut tx, membership.id)
                .await
                .expect("the row")
                .expect("kept");
            let left_at = left.deleted_at.expect("deleted");
            assert_eq!(left.deleted_by_id, Some(alice.id));
            let rows = app_posts_communities::replication::list_of_membership_deleted_at(
                &mut tx,
                membership.id,
                left_at,
            )
            .await
            .expect("the rows");
            assert_eq!(
                rows.len(),
                1,
                "roses left with her, weeds was removed before"
            );
            assert_eq!(rows[0].post_id, roses.id);
            tx.commit().await.expect("committed");
            let error = leave(&deployment, &alice, gardening.id)
                .await
                .expect_err("already left");
            assert!(matches!(error, MembershipError::NotFound { .. }));

            let rejoined = join(&deployment, &alice, gardening.id)
                .await
                .expect("rejoined");
            assert_eq!(rejoined.id, membership.id, "the same row");
            assert_eq!(rejoined.version, 3);
            assert!(rejoined.deleted_at.is_none());
            assert_eq!(bodies(&deployment, &bob).await, vec!["bob's", "roses"]);
            assert_eq!(
                join(&deployment, &alice, gardening.id)
                    .await
                    .expect("joined again")
                    .version,
                3,
                "joining when in is nothing"
            );
            let _ = bobs;
        })
        .await;
    }

    #[tokio::test]
    async fn memberships_rejoin_leaves_a_post_deleted_meanwhile_deleted() {
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
            join(&deployment, &bob, gardening.id).await.expect("joined");
            let roses = posts::create(&deployment, &alice, "roses", &[gardening.id])
                .await
                .expect("a post");
            leave(&deployment, &alice, gardening.id)
                .await
                .expect("left");
            posts::delete(&deployment, &alice, roses.id)
                .await
                .expect("deleted");

            join(&deployment, &alice, gardening.id)
                .await
                .expect("rejoined");
            assert!(bodies(&deployment, &bob).await.is_empty());
            let mut tx = deployment.begin().await.expect("a transaction");
            let row = app_posts_communities::replication::get_by_post_and_community(
                &mut tx,
                roses.id,
                gardening.id,
            )
            .await
            .expect("the row")
            .expect("kept");
            let post = crate::data::app_posts::replication::get(&mut tx, roses.id)
                .await
                .expect("the post")
                .expect("kept");
            assert_eq!(
                row.deleted_at, post.deleted_at,
                "the post's delete, not the leave's"
            );
        })
        .await;
    }

    #[tokio::test]
    async fn memberships_join_needs_a_community() {
        with_each_database(|database| async move {
            let deployment = server(database);
            let alice = users::create(&deployment, "alice@example.com", "Alice")
                .await
                .expect("a user");
            let error = join(&deployment, &alice, Uuid::now_v7())
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
