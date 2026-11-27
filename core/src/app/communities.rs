//! Communities: the soft boundary inside a deployment, see
//! `docs/design/offline-sync.md`. A user joins communities, see
//! `memberships`, and an app's rows are in communities, each table by
//! its own rule. Any user creates a community and joins it as
//! they do; every user sees every community, so a device can list them
//! and join, but a trial user's, which nobody else sees or joins, see
//! `trial_users`. Any live member renames or deletes one, `member` being the
//! only role yet. A delete takes every membership in the community along,
//! and every table's rows in it, all at the one moment, see
//! `SyncedTable::delete_with_community`.

use crate::data::app_communities::{self, AcceptedCommunity, Community, TABLE};
use crate::data::app_users_communities;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;
use uuid::Uuid;

use crate::app::memberships::{self, MembershipError};
use crate::data::app_events::{self, Conflicts, Event, EventKind, Fields};
use crate::data::app_users::User;
use crate::data::app_users_communities::Membership;
use crate::database::Transaction;
use crate::deployment::{Deployment, Role, now};
use crate::helpers::field_errors::FieldErrors;
use crate::helpers::json;
use crate::sync::TableRow;
use crate::sync::VersionRange;
use crate::sync::events::{self, Answer, AnswerStatus};
use crate::sync::synced_table::{CurrentFields, LocalCreate, SyncedTable, TableError};
use crate::sync::tables;

const CREATE_FIELDS: &[&str] = &["name"];
pub(crate) const UPDATE_FIELDS: &[&str] = &["name", "deleted_at"];

/// A community as the pages show it: with the user's membership when
/// they are in it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommunityAndMembership {
    pub community: Community,
    pub membership: Option<Membership>,
}

#[derive(Debug, Error)]
pub enum CommunityError {
    #[error("community {id} not found")]
    NotFound { id: Uuid },
    #[error("only a member can change a community")]
    NotAMember,
    #[error("{0}")]
    Invalid(FieldErrors),
    #[error("{0}")]
    Malformed(String),
    #[error("the change was refused: {0}")]
    Rejected(String),
    #[error(transparent)]
    Membership(#[from] MembershipError),
    #[error(transparent)]
    Record(#[from] crate::sync::events::RecordError),
    /// A synced table's own module failed.
    #[error(transparent)]
    Table(#[from] TableError),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl From<FieldErrors> for CommunityError {
    fn from(errors: FieldErrors) -> CommunityError {
        CommunityError::Invalid(errors)
    }
}

impl From<serde_json::Error> for CommunityError {
    fn from(error: serde_json::Error) -> CommunityError {
        CommunityError::Malformed(error.to_string())
    }
}

/// Every community the user sees, by name, each with their membership
/// when they are in it: all of them but a trial user's, which only
/// that trial user sees, see `trial_users`.
pub async fn list(
    deployment: &Deployment,
    user: &User,
) -> Result<Vec<CommunityAndMembership>, CommunityError> {
    let mut tx = deployment.begin().await?;
    let communities = app_communities::list(&mut tx, user.id).await?;
    let memberships =
        app_users_communities::list(&mut tx, app_users_communities::Filter::User(user.id)).await?;
    tx.commit().await?;
    Ok(communities
        .into_iter()
        .map(|community| CommunityAndMembership {
            membership: memberships
                .iter()
                .find(|membership| membership.community_id == community.id)
                .cloned(),
            community,
        })
        .collect())
}

/// One community by its id, unless deleted, with the user's membership
/// when they are in it.
pub async fn get(
    deployment: &Deployment,
    user: &User,
    id: Uuid,
) -> Result<CommunityAndMembership, CommunityError> {
    let mut tx = deployment.begin().await?;
    let community = app_communities::get(&mut tx, user.id, id)
        .await?
        .ok_or(CommunityError::NotFound { id })?;
    let membership = memberships::get_live_in(&mut tx, user.id, id).await?;
    tx.commit().await?;
    Ok(CommunityAndMembership {
        community,
        membership,
    })
}

/// Makes a community, which the user joins.
pub async fn create(
    deployment: &Deployment,
    user: &User,
    name: &str,
) -> Result<Community, CommunityError> {
    let mut tx = deployment.begin().await?;
    let community = create_in(&mut tx, deployment.role, user, name).await?;
    tx.commit().await?;
    Ok(community)
}

/// `create` in the caller's transaction, for the community a new user
/// starts with, see `accounts`.
pub(crate) async fn create_in(
    tx: &mut Transaction,
    role: Role,
    user: &User,
    name: &str,
) -> Result<Community, CommunityError> {
    let name = app_communities::checked_name(name)?;
    let event = Event::create(
        TABLE,
        Uuid::now_v7(),
        json::object_fields(json!({ "name": name })),
        now(),
    );
    let community = apply_in(tx, role, user, &event).await?.community()?;
    memberships::create_in(tx, role, user, community.id).await?;
    Ok(community)
}

/// Writes a new name over a community the user is a member of.
pub async fn update(
    deployment: &Deployment,
    user: &User,
    id: Uuid,
    name: &str,
) -> Result<Community, CommunityError> {
    let name = app_communities::checked_name(name)?;
    let mut tx = deployment.begin().await?;
    let community = get_as_member(&mut tx, user, id).await?;
    if community.name == name {
        return Ok(community);
    }
    let event = Event::update(
        TABLE,
        id,
        community.version,
        json::object_fields(json!({ "name": community.name })),
        json::object_fields(json!({ "name": name })),
        now(),
    );
    let community = apply_in(&mut tx, deployment.role, user, &event)
        .await?
        .community()?;
    tx.commit().await?;
    Ok(community)
}

/// Deletes a community the user is a member of: a soft delete, which
/// takes every membership in it along.
pub async fn delete(deployment: &Deployment, user: &User, id: Uuid) -> Result<(), CommunityError> {
    let mut tx = deployment.begin().await?;
    let community = get_as_member(&mut tx, user, id).await?;
    let event = Event::update(
        TABLE,
        id,
        community.version,
        json::object_fields(json!({ "deleted_at": null })),
        json::object_fields(json!({ "deleted_at": now() })),
        now(),
    );
    apply_in(&mut tx, deployment.role, user, &event)
        .await?
        .community()?;
    tx.commit().await?;
    Ok(())
}

/// Applies an event of the user's to its community and answers it.
pub async fn apply_event(
    deployment: &Deployment,
    user: &User,
    event: &Event,
) -> Result<Answer, CommunityError> {
    let mut tx = deployment.begin().await?;
    let applied = apply_in(&mut tx, deployment.role, user, event).await?;
    tx.commit().await?;
    Ok(applied.answer)
}

/// The community as it travels on the wire.
pub fn table_row(community: &Community) -> Result<TableRow, CommunityError> {
    Ok(TableRow {
        table: TABLE.to_string(),
        row: serde_json::to_value(community)?,
    })
}

/// Writes the server's row over the device's, unless an event of the
/// device's for it is still pending; true when it was written.
pub(crate) async fn apply_server_row(
    tx: &mut Transaction,
    row: &Value,
) -> Result<bool, CommunityError> {
    let community: Community = serde_json::from_value(row.clone())?;
    if app_events::has_pending(tx, TABLE, community.id).await? {
        return Ok(false);
    }
    app_communities::replication::upsert(&mut *tx, &community).await?;
    Ok(true)
}

pub(crate) async fn delete_row(tx: &mut Transaction, id: Uuid) -> Result<(), CommunityError> {
    Ok(app_communities::maintenance::delete(tx, id).await?)
}

/// A page of the rows the server wrote after the cursor, deleted or not.
pub(crate) async fn list_synced_after(
    tx: &mut Transaction,
    user: &User,
    range: &VersionRange,
    limit: usize,
) -> Result<Vec<Community>, CommunityError> {
    Ok(app_communities::replication::list_synced_after(tx, user.id, range, limit).await?)
}

pub(crate) async fn list_by_ids(
    tx: &mut Transaction,
    ids: &[Uuid],
) -> Result<Vec<Community>, CommunityError> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    Ok(app_communities::replication::list_by_ids(tx, ids).await?)
}

pub(crate) async fn get_in(
    tx: &mut Transaction,
    id: Uuid,
) -> Result<Option<Community>, CommunityError> {
    Ok(app_communities::replication::get(tx, id).await?)
}

async fn get_live(tx: &mut Transaction, id: Uuid) -> Result<Community, CommunityError> {
    match get_in(tx, id).await? {
        Some(community) if community.deleted_at.is_none() => Ok(community),
        _ => Err(CommunityError::NotFound { id }),
    }
}

/// The live community, if the user is a live member of it.
async fn get_as_member(
    tx: &mut Transaction,
    user: &User,
    id: Uuid,
) -> Result<Community, CommunityError> {
    let community = get_live(tx, id).await?;
    if memberships::get_live_in(tx, user.id, id).await?.is_none() {
        return Err(CommunityError::NotAMember);
    }
    Ok(community)
}

/// An event applied: the answer, and the community as it now stands.
pub(crate) struct Applied {
    pub answer: Answer,
    pub community: Option<Community>,
}

impl Applied {
    fn community(self) -> Result<Community, CommunityError> {
        match (self.answer.status, self.community) {
            (AnswerStatus::Rejected, _) => Err(CommunityError::Rejected(
                self.answer.reason.unwrap_or_else(|| "rejected".to_string()),
            )),
            (_, Some(community)) => Ok(community),
            (_, None) => Err(CommunityError::NotFound { id: self.answer.id }),
        }
    }
}

/// Applies the event in the transaction: a create writes the row; an
/// update, from a live member only, merges into the row as a post's
/// does, and a delete takes the memberships along.
pub(crate) async fn apply_in(
    tx: &mut Transaction,
    role: Role,
    user: &User,
    event: &Event,
) -> Result<Applied, CommunityError> {
    crate::data::app_sync_versions::lock_for_role(tx, role).await?;
    if role == Role::Server
        && let Some(recorded) = app_events::get(tx, event.id).await?
    {
        let community = get_in(tx, event.row_id).await?;
        let server = community.as_ref().map(table_row).transpose()?;
        return Ok(Applied {
            answer: recorded.answer(server),
            community,
        });
    }

    let allowed = match event.kind {
        EventKind::Create => CREATE_FIELDS,
        EventKind::Update => UPDATE_FIELDS,
    };
    if let Some(reason) = events::not_editable(&event.changes, allowed) {
        let community = get_in(tx, event.row_id).await?;
        return rejected(event, reason, community);
    }

    let now = now();
    match event.kind {
        EventKind::Create => {
            if let Some(existing) = get_in(tx, event.row_id).await? {
                return rejected(event, "exists", Some(existing));
            }
            let (version, last_synced_at) = match role {
                Role::Server => (1, Some(now)),
                Role::Device => (0, None),
            };
            let blank = Community {
                id: event.row_id,
                name: String::new(),
                inserted_at: event.row_inserted_at(now),
                updated_at: now,
                version,
                last_synced_at,
                global_sync_version: None,
                deleted_at: None,
                deleted_by_id: None,
            };
            let accepted = blank
                .with_fields(&event.changes)
                .and_then(|community| Ok(AcceptedCommunity::new(community)?));
            let mut community = match accepted {
                Ok(community) => community,
                Err(error) => return rejected(event, format!("invalid: {error}"), None),
            };
            community.record_sync_change(tx, role).await?;
            app_communities::create(&mut *tx, &community).await?;
            finish(
                tx,
                role,
                user,
                event,
                Conflicts::new(),
                community.into_row(),
            )
            .await
        }
        EventKind::Update => {
            let Some(community) = get_in(tx, event.row_id).await? else {
                return rejected(event, "unknown_community", None);
            };
            if memberships::get_live_in(tx, user.id, community.id)
                .await?
                .is_none()
            {
                return rejected(event, "not_a_member", Some(community));
            }
            let (changes, conflicts) = events::merge(
                &community.fields(UPDATE_FIELDS)?,
                community.version,
                event.base_version,
                &event.base,
                &event.changes,
            );
            if changes.is_empty() {
                return finish(tx, role, user, event, conflicts, community).await;
            }
            let mut updated = match community.with_fields(&changes) {
                Ok(updated) => updated,
                Err(error) => {
                    return rejected(event, format!("invalid: {error}"), Some(community));
                }
            };
            updated.updated_at = now;
            if changes.contains_key("deleted_at") {
                updated.deleted_by_id = updated.deleted_at.map(|_| user.id);
            }
            if role == Role::Server {
                updated.version += 1;
                updated.last_synced_at = Some(now);
            }
            let mut updated = match AcceptedCommunity::new(updated) {
                Ok(updated) => updated,
                Err(error) => {
                    return rejected(event, format!("invalid: {error}"), Some(community));
                }
            };
            updated.record_sync_change(tx, role).await?;
            app_communities::update(&mut *tx, &updated).await?;
            let updated = updated.into_row();
            if changes.contains_key("deleted_at") && updated.deleted_at.is_some() {
                memberships::delete_with_community(tx, role, &updated).await?;
                for table in tables::list().iter().rev() {
                    table.delete_with_community(tx, role, &updated).await?;
                }
            }
            finish(tx, role, user, event, conflicts, updated).await
        }
    }
}

/// Answers an applied event and does the role's part.
async fn finish(
    tx: &mut Transaction,
    role: Role,
    user: &User,
    event: &Event,
    conflicts: Conflicts,
    community: Community,
) -> Result<Applied, CommunityError> {
    let answer = Answer::applied(
        event.id,
        community.version,
        conflicts,
        table_row(&community)?,
    );
    match role {
        Role::Server => events::record(tx, user.id, event, &answer).await?,
        Role::Device => events::queue(tx, user.id, event).await?,
    }
    Ok(Applied {
        answer,
        community: Some(community),
    })
}

fn rejected(
    event: &Event,
    reason: impl Into<String>,
    community: Option<Community>,
) -> Result<Applied, CommunityError> {
    let server = community.as_ref().map(table_row).transpose()?;
    Ok(Applied {
        answer: Answer::rejected(event.id, reason, server),
        community,
    })
}

impl Community {
    /// The named fields as they travel on the wire.
    pub(crate) fn fields(&self, names: &[&str]) -> Result<Fields, CommunityError> {
        let mut all = json::object_fields(serde_json::to_value(self)?);
        all.retain(|name, _| names.contains(&name.as_str()));
        Ok(all)
    }

    /// The community with the fields written over it, not yet checked.
    fn with_fields(&self, changes: &Fields) -> Result<Community, CommunityError> {
        let mut all = json::object_fields(serde_json::to_value(self)?);
        for (name, value) in changes {
            all.insert(name.clone(), value.clone());
        }
        Ok(serde_json::from_value(Value::Object(all))?)
    }
}

/// Communities as a synced table, see `synced_tables`.
pub struct Synced;

#[async_trait]
impl SyncedTable for Synced {
    fn table(&self) -> &'static str {
        TABLE
    }

    fn owner_column(&self) -> Option<&'static str> {
        None
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
        let communities = list_synced_after(tx, user, range, limit).await?;
        Ok(communities
            .iter()
            .map(table_row)
            .collect::<Result<_, _>>()?)
    }

    async fn parents(
        &self,
        _tx: &mut Transaction,
        _row: &TableRow,
    ) -> Result<Vec<TableRow>, TableError> {
        Ok(Vec::new())
    }

    async fn apply_server_row(
        &self,
        tx: &mut Transaction,
        _user: &User,
        row: &TableRow,
    ) -> Result<bool, TableError> {
        Ok(apply_server_row(tx, &row.row).await?)
    }

    async fn delete_row(&self, tx: &mut Transaction, id: Uuid) -> Result<(), TableError> {
        Ok(delete_row(tx, id).await?)
    }

    async fn local_user_creates(
        &self,
        tx: &mut Transaction,
        user: &User,
    ) -> Result<Vec<LocalCreate>, TableError> {
        let communities = app_communities::list(&mut *tx, user.id).await?;
        Ok(communities
            .into_iter()
            .map(|community| LocalCreate {
                id: community.id,
                inserted_at: community.inserted_at,
                parents: Vec::new(),
                changes: json::object_fields(json!({ "name": community.name })),
            })
            .collect())
    }

    async fn label(&self, tx: &mut Transaction, id: Uuid) -> Result<Option<String>, TableError> {
        Ok(get_in(tx, id).await?.map(|community| community.name))
    }

    async fn current_fields(
        &self,
        tx: &mut Transaction,
        id: Uuid,
    ) -> Result<Option<CurrentFields>, TableError> {
        let Some(community) = get_in(tx, id).await? else {
            return Ok(None);
        };
        if community.deleted_at.is_some() {
            return Ok(None);
        }
        Ok(Some(CurrentFields {
            version: community.version,
            fields: community.fields(UPDATE_FIELDS)?,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::users;
    use crate::database::{Database, with_each_database};
    use crate::sync::test_notes;

    fn server(database: Database) -> Deployment {
        Deployment::new(database, Role::Server)
    }

    async fn user(deployment: &Deployment, email: &str) -> User {
        users::create(deployment, email, email.split('@').next().unwrap_or(email))
            .await
            .expect("a user")
    }

    #[tokio::test]
    async fn communities_create_in_rolls_back_membership_and_events_with_community() {
        with_each_database(|database| async move {
            let deployment = server(database);
            let alice = user(&deployment, "alice@example.com").await;
            let mut tx = deployment.begin().await.expect("transaction");
            let before = app_events::count(&mut tx, app_events::EventStatus::Applied)
                .await
                .expect("count");
            tx.commit().await.expect("commit");
            let mut tx = deployment.begin().await.expect("transaction");
            let community = create_in(&mut tx, Role::Server, &alice, "Rolled back")
                .await
                .expect("community");
            let membership = memberships::get_live_in(&mut tx, alice.id, community.id)
                .await
                .expect("read")
                .expect("membership");
            drop(tx);
            let mut tx = deployment.begin().await.expect("transaction");
            assert!(
                app_communities::replication::get(&mut tx, community.id)
                    .await
                    .expect("read")
                    .is_none()
            );
            assert!(
                app_users_communities::replication::get(&mut tx, membership.id)
                    .await
                    .expect("read")
                    .is_none()
            );
            assert_eq!(
                app_events::count(&mut tx, app_events::EventStatus::Applied)
                    .await
                    .expect("count"),
                before
            );
        })
        .await;
    }

    #[tokio::test]
    async fn communities_records_exclude_deleted_relationships_but_keep_protocol_rows() {
        with_each_database(|database| async move {
            let deployment = server(database);
            let alice = user(&deployment, "alice@example.com").await;
            let community = create(&deployment, &alice, "Gardening")
                .await
                .expect("community");
            let note = test_notes::create(&deployment, &alice, "roses", Some(community.id))
                .await
                .expect("note");
            delete(&deployment, &alice, community.id)
                .await
                .expect("delete");
            let mut tx = deployment.begin().await.expect("transaction");
            assert!(
                app_communities::get(&mut tx, alice.id, community.id)
                    .await
                    .expect("read")
                    .is_none()
            );
            assert!(
                app_users_communities::get(&mut tx, alice.id, community.id)
                    .await
                    .expect("read")
                    .is_none()
            );
            assert!(
                app_communities::replication::get(&mut tx, community.id)
                    .await
                    .expect("read")
                    .expect("tombstone")
                    .deleted_at
                    .is_some()
            );
            assert!(
                app_users_communities::replication::get_by_user_and_community(
                    &mut tx,
                    alice.id,
                    community.id
                )
                .await
                .expect("read")
                .expect("tombstone")
                .deleted_at
                .is_some()
            );
            assert!(
                test_notes::get_in(&mut tx, note.id)
                    .await
                    .expect("read")
                    .expect("tombstone")
                    .deleted_at
                    .is_some()
            );
        })
        .await;
    }

    #[tokio::test]
    async fn communities_create_joins_the_maker_and_lists_for_everyone() {
        with_each_database(|database| async move {
            let deployment = server(database);
            let alice = user(&deployment, "alice@example.com").await;
            let bob = user(&deployment, "bob@example.com").await;
            let gardening = create(&deployment, &alice, " Gardening ")
                .await
                .expect("a community");
            assert_eq!(gardening.name, "Gardening");
            assert_eq!(gardening.version, 1);

            let alices = list(&deployment, &alice).await.expect("the list");
            assert_eq!(alices.len(), 1);
            assert_eq!(alices[0].community, gardening);
            let membership = alices[0].membership.as_ref().expect("joined");
            assert_eq!(membership.user_id, alice.id);
            assert_eq!(membership.role, app_users_communities::ROLE_MEMBER);

            let bobs = list(&deployment, &bob).await.expect("the list");
            assert_eq!(bobs[0].community, gardening);
            assert!(bobs[0].membership.is_none());

            let error = create(&deployment, &alice, " ").await.expect_err("blank");
            assert!(
                matches!(error, CommunityError::Invalid(errors) if errors == FieldErrors::one("name", "is blank"))
            );
            create(&deployment, &alice, &"é".repeat(app_communities::MAX_NAME_LENGTH))
                .await
                .expect("the maximum, counted in characters");
            let error = create(&deployment, &alice, &"é".repeat(app_communities::MAX_NAME_LENGTH + 1))
                .await
                .expect_err("one more");
            assert!(
                matches!(error, CommunityError::Invalid(errors) if errors == FieldErrors::one("name", "has 80 characters at most"))
            );
        })
        .await;
    }

    #[tokio::test]
    async fn communities_get_with_the_users_membership() {
        with_each_database(|database| async move {
            let deployment = server(database);
            let alice = user(&deployment, "alice@example.com").await;
            let bob = user(&deployment, "bob@example.com").await;
            let gardening = create(&deployment, &alice, "Gardening")
                .await
                .expect("a community");

            let alices = get(&deployment, &alice, gardening.id).await.expect("found");
            assert_eq!(alices.community, gardening);
            assert!(alices.membership.is_some());
            let bobs = get(&deployment, &bob, gardening.id).await.expect("found");
            assert!(bobs.membership.is_none());

            let error = get(&deployment, &alice, Uuid::now_v7())
                .await
                .expect_err("missing");
            assert!(matches!(error, CommunityError::NotFound { .. }));
        })
        .await;
    }

    #[tokio::test]
    async fn communities_update_by_a_member_only() {
        with_each_database(|database| async move {
            let deployment = server(database);
            let alice = user(&deployment, "alice@example.com").await;
            let bob = user(&deployment, "bob@example.com").await;
            let gardening = create(&deployment, &alice, "Gardening")
                .await
                .expect("a community");

            let renamed = update(&deployment, &alice, gardening.id, " Weeding ")
                .await
                .expect("renamed");
            assert_eq!(renamed.name, "Weeding");
            assert_eq!(renamed.version, 2);
            assert_eq!(renamed.inserted_at, gardening.inserted_at);
            let same = update(&deployment, &alice, gardening.id, "Weeding")
                .await
                .expect("unchanged");
            assert_eq!(same.version, 2);

            let error = update(&deployment, &alice, gardening.id, "")
                .await
                .expect_err("blank");
            assert!(matches!(error, CommunityError::Invalid(_)));
            let error = update(&deployment, &bob, gardening.id, "Bob's")
                .await
                .expect_err("not a member");
            assert!(matches!(error, CommunityError::NotAMember));

            memberships::create(&deployment, &bob, gardening.id)
                .await
                .expect("joined");
            let renamed = update(&deployment, &bob, gardening.id, "Bob's")
                .await
                .expect("any member renames");
            assert_eq!(renamed.name, "Bob's");
        })
        .await;
    }

    #[tokio::test]
    async fn communities_apply_event_refuses_a_non_members_change_with_the_servers_row() {
        with_each_database(|database| async move {
            let deployment = server(database);
            let alice = user(&deployment, "alice@example.com").await;
            let bob = user(&deployment, "bob@example.com").await;
            let gardening = create(&deployment, &alice, "Gardening")
                .await
                .expect("a community");
            let event = Event::update(
                TABLE,
                gardening.id,
                1,
                json::object_fields(json!({ "name": "Gardening" })),
                json::object_fields(json!({ "name": "Weeding" })),
                now(),
            );

            let answer = apply_event(&deployment, &bob, &event)
                .await
                .expect("answered");
            assert_eq!(answer.status, AnswerStatus::Rejected);
            assert_eq!(answer.reason.as_deref(), Some("not_a_member"));
            assert_eq!(
                answer.server.expect("the row").row["name"],
                json!("Gardening")
            );

            let answer = apply_event(&deployment, &alice, &event)
                .await
                .expect("answered");
            assert_eq!(answer.status, AnswerStatus::Applied);
            let again = apply_event(&deployment, &alice, &event)
                .await
                .expect("answered again");
            assert_eq!(again.version, answer.version, "a retry applies once");
        })
        .await;
    }

    #[tokio::test]
    async fn communities_delete_takes_every_membership_and_every_row_in_it_along() {
        with_each_database(|database| async move {
            let deployment = server(database);
            let alice = user(&deployment, "alice@example.com").await;
            let bob = user(&deployment, "bob@example.com").await;
            let carol = user(&deployment, "carol@example.com").await;
            let gardening = create(&deployment, &alice, "Gardening")
                .await
                .expect("a community");
            memberships::create(&deployment, &bob, gardening.id)
                .await
                .expect("joined");
            let note = test_notes::create(&deployment, &alice, "roses", Some(gardening.id))
                .await
                .expect("a note");

            let error = delete(&deployment, &carol, gardening.id)
                .await
                .expect_err("not a member");
            assert!(matches!(error, CommunityError::NotAMember));

            delete(&deployment, &bob, gardening.id)
                .await
                .expect("deleted");
            let error = get(&deployment, &alice, gardening.id)
                .await
                .expect_err("gone");
            assert!(matches!(error, CommunityError::NotFound { .. }));
            assert!(list(&deployment, &alice).await.expect("none").is_empty());
            for member in [&alice, &bob] {
                assert!(
                    test_notes::list(&deployment, member)
                        .await
                        .expect("the notes")
                        .is_empty(),
                    "the note went with the community"
                );
            }

            let mut tx = deployment.begin().await.expect("a transaction");
            let row = get_in(&mut tx, gardening.id)
                .await
                .expect("the row")
                .expect("kept");
            assert_eq!(row.deleted_by_id, Some(bob.id));
            for member in [&alice, &bob] {
                let membership = app_users_communities::replication::get_by_user_and_community(
                    &mut tx,
                    member.id,
                    gardening.id,
                )
                .await
                .expect("the row")
                .expect("kept");
                assert_eq!(membership.deleted_at, row.deleted_at, "the one moment");
                assert_eq!(membership.deleted_by_id, Some(bob.id));
            }
            let note = test_notes::get_in(&mut tx, note.id)
                .await
                .expect("the note")
                .expect("kept");
            assert_eq!(note.deleted_at, row.deleted_at, "the one moment");
            assert_eq!(note.deleted_by_id, Some(bob.id));
            tx.commit().await.expect("committed");

            let error = memberships::create(&deployment, &alice, gardening.id)
                .await
                .expect_err("nothing to rejoin");
            assert!(
                matches!(error, MembershipError::Rejected(reason) if reason == "unknown_community")
            );
        })
        .await;
    }
}
