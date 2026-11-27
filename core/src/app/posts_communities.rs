//! A post in a community: a row for each community a post's owner put it
//! in, under the owner's membership there. Only the owner adds their
//! post or removes it, and only to a community they are in. Removing
//! soft-deletes the row and adding back revives it. Deleting the post
//! deletes its rows with it, and leaving a community deletes the member's
//! rows there with the membership, at the one moment, so rejoining
//! revives exactly those, unless their post was deleted meanwhile.
//!
//! The server decides these rows as it does memberships: a device's event
//! is applied through the same add and remove, and the server's row comes
//! back.

use crate::data::app_posts_communities::{self, PostCommunity, TABLE};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use thiserror::Error;
use uuid::Uuid;

use crate::app::memberships::{self, id_field};
use crate::app::{communities, posts};
use crate::data::app_events::{self, Conflicts, Event, EventKind};
use crate::data::app_posts;
use crate::data::app_posts::Post;
use crate::data::app_users::User;
use crate::data::app_users_communities::Membership;
use crate::database::Transaction;
use crate::deployment::{Deployment, Role, now};
use crate::helpers::json;
use crate::sync::TableRow;
use crate::sync::VersionRange;
use crate::sync::events::{self, Answer, AnswerStatus};
use crate::sync::synced_table::{LocalCreate, SyncedTable, TableError};

const CREATE_FIELDS: &[&str] = &["post_id", "community_id", "users_communities_id"];
const UPDATE_FIELDS: &[&str] = &["deleted_at"];

#[derive(Debug, Error)]
pub enum PostCommunityError {
    #[error("post {id} not found")]
    UnknownPost { id: Uuid },
    #[error("only the owner puts a post in a community")]
    NotOwner,
    #[error("not a member of community {community_id}")]
    NotAMember { community_id: Uuid },
    #[error("the post is not in community {community_id}")]
    NotFound { community_id: Uuid },
    #[error("{0}")]
    Invalid(String),
    #[error("the change was refused: {0}")]
    Rejected(String),
    #[error(transparent)]
    Record(#[from] crate::sync::events::RecordError),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl From<serde_json::Error> for PostCommunityError {
    fn from(error: serde_json::Error) -> PostCommunityError {
        PostCommunityError::Invalid(error.to_string())
    }
}

/// Puts the user's post in the community, or back in it.
pub async fn add(
    deployment: &Deployment,
    user: &User,
    post_id: Uuid,
    community_id: Uuid,
) -> Result<PostCommunity, PostCommunityError> {
    let mut tx = deployment.begin().await?;
    let row = add_in(&mut tx, deployment.role, user, post_id, community_id).await?;
    tx.commit().await?;
    Ok(row)
}

/// Takes the user's post out of the community.
pub async fn remove(
    deployment: &Deployment,
    user: &User,
    post_id: Uuid,
    community_id: Uuid,
) -> Result<(), PostCommunityError> {
    let mut tx = deployment.begin().await?;
    remove_in(&mut tx, deployment.role, user, post_id, community_id).await?;
    tx.commit().await?;
    Ok(())
}

/// Applies an event of the user's to a row and answers it.
pub async fn apply_event(
    deployment: &Deployment,
    user: &User,
    event: &Event,
) -> Result<Answer, PostCommunityError> {
    let mut tx = deployment.begin().await?;
    let applied = apply_in(&mut tx, deployment.role, user, event).await?;
    tx.commit().await?;
    Ok(applied.answer)
}

pub fn table_row(row: &PostCommunity) -> Result<TableRow, PostCommunityError> {
    Ok(TableRow {
        table: TABLE.to_string(),
        row: serde_json::to_value(row)?,
    })
}

pub(crate) async fn add_in(
    tx: &mut Transaction,
    role: Role,
    user: &User,
    post_id: Uuid,
    community_id: Uuid,
) -> Result<PostCommunity, PostCommunityError> {
    let post = own_live_post(tx, user, post_id).await?;
    let Some(membership) = memberships::get_live_in(tx, user.id, community_id)
        .await
        .map_err(invalid)?
    else {
        return Err(PostCommunityError::NotAMember { community_id });
    };
    let event = match app_posts_communities::replication::get_by_post_and_community(
        &mut *tx,
        post.id,
        community_id,
    )
    .await?
    {
        None => Event::create(
            TABLE,
            Uuid::now_v7(),
            json::object_fields(json!({
                "post_id": post.id,
                "community_id": community_id,
                "users_communities_id": membership.id
            })),
            now(),
        ),
        Some(row) if row.deleted_at.is_none() => return Ok(row),
        Some(removed) => Event::update(
            TABLE,
            removed.id,
            removed.version,
            json::object_fields(json!({ "deleted_at": removed.deleted_at })),
            json::object_fields(json!({ "deleted_at": null })),
            now(),
        ),
    };
    apply_in(tx, role, user, &event).await?.row()
}

pub(crate) async fn remove_in(
    tx: &mut Transaction,
    role: Role,
    user: &User,
    post_id: Uuid,
    community_id: Uuid,
) -> Result<PostCommunity, PostCommunityError> {
    let post = own_live_post(tx, user, post_id).await?;
    let row = match app_posts_communities::replication::get_by_post_and_community(
        &mut *tx,
        post.id,
        community_id,
    )
    .await?
    {
        Some(row) if row.deleted_at.is_none() => row,
        _ => return Err(PostCommunityError::NotFound { community_id }),
    };
    let event = Event::update(
        TABLE,
        row.id,
        row.version,
        json::object_fields(json!({ "deleted_at": null })),
        json::object_fields(json!({ "deleted_at": now() })),
        now(),
    );
    apply_in(tx, role, user, &event).await?.row()
}

/// Makes the post's communities exactly these: adds what is missing and
/// removes the rest.
pub(crate) async fn set_in(
    tx: &mut Transaction,
    role: Role,
    user: &User,
    post_id: Uuid,
    community_ids: &[Uuid],
) -> Result<(), PostCommunityError> {
    let current =
        app_posts_communities::list(&mut *tx, app_posts_communities::Filter::Posts(&[post_id]))
            .await?;
    for row in &current {
        if !community_ids.contains(&row.community_id) {
            remove_in(tx, role, user, post_id, row.community_id).await?;
        }
    }
    for community_id in community_ids {
        if !current.iter().any(|row| row.community_id == *community_id) {
            add_in(tx, role, user, post_id, *community_id).await?;
        }
    }
    Ok(())
}

/// The live rows of the posts.
pub(crate) async fn list_live_of_posts(
    tx: &mut Transaction,
    post_ids: &[Uuid],
) -> Result<Vec<PostCommunity>, PostCommunityError> {
    if post_ids.is_empty() {
        return Ok(Vec::new());
    }
    Ok(app_posts_communities::list(tx, app_posts_communities::Filter::Posts(post_ids)).await?)
}

/// Deletes the post's live rows with it, at the post's moment.
pub(crate) async fn delete_with_post(
    tx: &mut Transaction,
    role: Role,
    post: &Post,
) -> Result<(), PostCommunityError> {
    for mut row in
        app_posts_communities::list(&mut *tx, app_posts_communities::Filter::Posts(&[post.id]))
            .await?
    {
        row.deleted_at = post.deleted_at;
        row.deleted_by_id = post.deleted_by_id;
        write_changed(tx, role, &mut row).await?;
    }
    Ok(())
}

/// Deletes the member's live rows in the community with their
/// membership, at its moment.
async fn delete_with_membership(
    tx: &mut Transaction,
    role: Role,
    membership: &Membership,
) -> Result<(), PostCommunityError> {
    for mut row in app_posts_communities::list(
        &mut *tx,
        app_posts_communities::Filter::Membership(membership.id),
    )
    .await?
    {
        row.deleted_at = membership.deleted_at;
        row.deleted_by_id = membership.deleted_by_id;
        write_changed(tx, role, &mut row).await?;
    }
    Ok(())
}

/// Revives the rows that left with the membership at `left_at`, unless
/// their post was deleted meanwhile, in which case a row takes the
/// post's delete instead.
async fn revive_with_membership(
    tx: &mut Transaction,
    role: Role,
    membership: &Membership,
    left_at: DateTime<Utc>,
) -> Result<(), PostCommunityError> {
    for mut row in app_posts_communities::replication::list_of_membership_deleted_at(
        &mut *tx,
        membership.id,
        left_at,
    )
    .await?
    {
        let post = crate::data::app_posts::replication::get(tx, row.post_id).await?;
        row.deleted_at = post.as_ref().and_then(|post| post.deleted_at);
        row.deleted_by_id = post.as_ref().and_then(|post| post.deleted_by_id);
        write_changed(tx, role, &mut row).await?;
    }
    Ok(())
}

/// Writes the server's row over the device's, unless an event of the
/// device's for it is still pending, or the device does not have its
/// post: a row taken out of a community before this device saw the
/// post; true when it was written.
pub(crate) async fn apply_server_row(
    tx: &mut Transaction,
    row: &Value,
) -> Result<bool, PostCommunityError> {
    let row: PostCommunity = serde_json::from_value(row.clone())?;
    if app_events::has_pending(tx, TABLE, row.id).await? {
        return Ok(false);
    }
    if crate::data::app_posts::replication::get(tx, row.post_id)
        .await?
        .is_none()
    {
        return Ok(false);
    }
    app_posts_communities::replication::upsert(&mut *tx, &row).await?;
    Ok(true)
}

pub(crate) async fn delete_row(tx: &mut Transaction, id: Uuid) -> Result<(), PostCommunityError> {
    Ok(app_posts_communities::maintenance::delete(tx, id).await?)
}

pub(crate) async fn get_in(
    tx: &mut Transaction,
    id: Uuid,
) -> Result<Option<PostCommunity>, PostCommunityError> {
    Ok(app_posts_communities::replication::get(tx, id).await?)
}

pub(crate) async fn list_synced_after(
    tx: &mut Transaction,
    user: &User,
    range: &VersionRange,
    limit: usize,
) -> Result<Vec<PostCommunity>, PostCommunityError> {
    Ok(app_posts_communities::replication::list_synced_after(tx, user.id, range, limit).await?)
}

/// A page of the community's live rows after the cursor: a backfill.
pub(crate) async fn list_live_of_community_after(
    tx: &mut Transaction,
    community_id: Uuid,
    range: &VersionRange,
    limit: usize,
) -> Result<Vec<PostCommunity>, PostCommunityError> {
    Ok(
        app_posts_communities::replication::list_live_of_community_after(
            tx,
            community_id,
            range,
            limit,
        )
        .await?,
    )
}

async fn own_live_post(
    tx: &mut Transaction,
    user: &User,
    post_id: Uuid,
) -> Result<Post, PostCommunityError> {
    match crate::data::app_posts::replication::get(tx, post_id).await? {
        Some(post) if post.deleted_at.is_none() && post.user_id == user.id => Ok(post),
        Some(post) if post.deleted_at.is_none() => {
            let _ = post;
            Err(PostCommunityError::NotOwner)
        }
        _ => Err(PostCommunityError::UnknownPost { id: post_id }),
    }
}

struct Applied {
    answer: Answer,
    row: Option<PostCommunity>,
}

impl Applied {
    fn row(self) -> Result<PostCommunity, PostCommunityError> {
        match (self.answer.status, self.row) {
            (AnswerStatus::Rejected, _) => Err(PostCommunityError::Rejected(
                self.answer.reason.unwrap_or_else(|| "rejected".to_string()),
            )),
            (_, Some(row)) => Ok(row),
            (_, None) => Err(PostCommunityError::Invalid("no row".to_string())),
        }
    }
}

/// Applies the event in the transaction, as what the person meant: a
/// create adds the post to the community, an update of `deleted_at`
/// removes it or adds it back, whatever the server already holds for the
/// pair. Only the owner's post, in a community they are in.
async fn apply_in(
    tx: &mut Transaction,
    role: Role,
    user: &User,
    event: &Event,
) -> Result<Applied, PostCommunityError> {
    if role == Role::Server
        && let Some(recorded) = app_events::get(tx, event.id).await?
    {
        let row = get_in(tx, event.row_id).await?;
        let server = row.as_ref().map(table_row).transpose()?;
        return Ok(Applied {
            answer: recorded.answer(server),
            row,
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
        let row = get_in(tx, event.row_id).await?;
        return rejected(event, reason, row);
    }

    let row = match event.kind {
        EventKind::Create => {
            let Some(post_id) = id_field(&event.changes, "post_id") else {
                return rejected(event, "unknown_post", None);
            };
            let Some(community_id) = id_field(&event.changes, "community_id") else {
                return rejected(event, "unknown_community", None);
            };
            let post = match own_live_post(tx, user, post_id).await {
                Ok(post) => post,
                Err(PostCommunityError::NotOwner) => return rejected(event, "not_owner", None),
                Err(PostCommunityError::UnknownPost { .. }) => {
                    return rejected(event, "unknown_post", None);
                }
                Err(error) => return Err(error),
            };
            let Some(membership) = memberships::get_live_in(tx, user.id, community_id)
                .await
                .map_err(invalid)?
            else {
                return rejected(event, "not_a_member", None);
            };
            match app_posts_communities::replication::get_by_post_and_community(
                &mut *tx,
                post.id,
                community_id,
            )
            .await?
            {
                Some(existing) if existing.deleted_at.is_none() => existing,
                Some(removed) => add_back(tx, role, removed, &membership, &post).await?,
                None => {
                    if let Some(existing) = get_in(tx, event.row_id).await? {
                        return rejected(event, "exists", Some(existing));
                    }
                    let now = now();
                    let (version, last_synced_at) = match role {
                        Role::Server => (1, Some(now)),
                        Role::Device => (0, None),
                    };
                    let row = PostCommunity {
                        id: event.row_id,
                        post_id: post.id,
                        community_id,
                        users_communities_id: membership.id,
                        inserted_at: event.row_inserted_at(now),
                        updated_at: now,
                        version,
                        last_synced_at,
                        global_sync_version: None,
                        deleted_at: None,
                        deleted_by_id: None,
                    };
                    app_posts_communities::create(&mut *tx, role, row).await?
                }
            }
        }
        EventKind::Update => {
            let Some(row) = get_in(tx, event.row_id).await? else {
                return rejected(event, "unknown_row", None);
            };
            let post = match crate::data::app_posts::replication::get(tx, row.post_id).await? {
                Some(post) if post.user_id == user.id => post,
                Some(_) => return rejected(event, "not_owner", Some(row)),
                None => return rejected(event, "unknown_post", Some(row)),
            };
            let deleted_at: Option<DateTime<Utc>> = match event.changes.get("deleted_at") {
                Some(value) => match serde_json::from_value(value.clone()) {
                    Ok(deleted_at) => deleted_at,
                    Err(error) => return rejected(event, format!("invalid: {error}"), Some(row)),
                },
                None => row.deleted_at,
            };
            match (row.deleted_at, deleted_at) {
                (Some(_), None) => {
                    if post.deleted_at.is_some() {
                        return rejected(event, "unknown_post", Some(row));
                    }
                    let Some(membership) = memberships::get_live_in(tx, user.id, row.community_id)
                        .await
                        .map_err(invalid)?
                    else {
                        return rejected(event, "not_a_member", Some(row));
                    };
                    add_back(tx, role, row, &membership, &post).await?
                }
                (None, Some(at)) => {
                    let mut removed = row;
                    removed.deleted_at = Some(at);
                    removed.deleted_by_id = Some(user.id);
                    write_changed(tx, role, &mut removed).await?;
                    removed
                }
                _ => row,
            }
        }
    };
    let answer = Answer::applied(event.id, row.version, Conflicts::new(), table_row(&row)?);
    match role {
        Role::Server => events::record(tx, user.id, event, &answer).await?,
        Role::Device => events::queue(tx, user.id, event).await?,
    }
    Ok(Applied {
        answer,
        row: Some(row),
    })
}

/// Revives the row under the owner's membership now, with the post's
/// delete if the post is deleted.
async fn add_back(
    tx: &mut Transaction,
    role: Role,
    mut row: PostCommunity,
    membership: &Membership,
    post: &Post,
) -> Result<PostCommunity, PostCommunityError> {
    row.users_communities_id = membership.id;
    row.deleted_at = post.deleted_at;
    row.deleted_by_id = post.deleted_by_id;
    write_changed(tx, role, &mut row).await?;
    Ok(row)
}

/// Writes the row over itself as the role does: the server increments
/// the version and sets the sync time.
async fn write_changed(
    tx: &mut Transaction,
    role: Role,
    row: &mut PostCommunity,
) -> Result<(), PostCommunityError> {
    let now = now();
    row.updated_at = now;
    if role == Role::Server {
        row.version += 1;
        row.last_synced_at = Some(now);
    }
    *row = app_posts_communities::update(&mut *tx, role, row.clone()).await?;
    Ok(())
}

fn rejected(
    event: &Event,
    reason: impl Into<String>,
    row: Option<PostCommunity>,
) -> Result<Applied, PostCommunityError> {
    let server = row.as_ref().map(table_row).transpose()?;
    Ok(Applied {
        answer: Answer::rejected(event.id, reason, server),
        row,
    })
}

fn invalid(error: memberships::MembershipError) -> PostCommunityError {
    match error {
        memberships::MembershipError::Database(error) => PostCommunityError::Database(error),
        other => PostCommunityError::Invalid(other.to_string()),
    }
}

/// Posts' community rows as a synced table, see `synced_tables`.
pub struct Synced;

#[async_trait]
impl SyncedTable for Synced {
    fn foreign_keys(&self) -> &'static [(&'static str, &'static str)] {
        &[
            ("post_id", "app_posts"),
            ("community_id", "app_communities"),
            ("users_communities_id", "app_users_communities"),
        ]
    }

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
        let rows = list_synced_after(tx, user, range, limit).await?;
        Ok(rows.iter().map(table_row).collect::<Result<_, _>>()?)
    }

    /// A live row brings its post along; a row taken out of a community
    /// brings none, since the user may never have seen the post.
    async fn parents(
        &self,
        tx: &mut Transaction,
        row: &TableRow,
    ) -> Result<Vec<TableRow>, TableError> {
        let row: PostCommunity = serde_json::from_value(row.row.clone())?;
        let mut parents = Vec::new();
        if row.deleted_at.is_none()
            && let Some(post) = app_posts::replication::get(tx, row.post_id).await?
        {
            parents.push(posts::table_row(&post)?);
        }
        if let Some(community) = communities::get_in(tx, row.community_id).await? {
            parents.push(communities::table_row(&community)?);
        }
        if let Some(membership) = memberships::get_in(tx, row.users_communities_id).await? {
            parents.push(memberships::table_row(&membership)?);
        }
        Ok(parents)
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
        let posts = app_posts::list(tx, app_posts::Filter::VisibleTo(user.id)).await?;
        let post_ids: Vec<Uuid> = posts.iter().map(|post| post.id).collect();
        if post_ids.is_empty() {
            return Ok(Vec::new());
        }
        let rows =
            app_posts_communities::list(&mut *tx, app_posts_communities::Filter::Posts(&post_ids))
                .await?;
        Ok(rows
            .into_iter()
            .map(|row| LocalCreate {
                id: row.id,
                inserted_at: row.inserted_at,
                parents: vec![row.post_id, row.community_id, row.users_communities_id],
                changes: json::object_fields(json!({
                    "post_id": row.post_id,
                    "community_id": row.community_id,
                    "users_communities_id": row.users_communities_id,
                })),
            })
            .collect())
    }

    async fn list_live_in_community_after(
        &self,
        tx: &mut Transaction,
        community_id: Uuid,
        range: &VersionRange,
        limit: usize,
    ) -> Result<Vec<TableRow>, TableError> {
        let rows = list_live_of_community_after(tx, community_id, range, limit).await?;
        Ok(rows.iter().map(table_row).collect::<Result<_, _>>()?)
    }

    /// A row is put in under its owner's membership, so it leaves with it.
    async fn delete_with_membership(
        &self,
        tx: &mut Transaction,
        role: Role,
        membership: &Membership,
    ) -> Result<(), TableError> {
        Ok(delete_with_membership(tx, role, membership).await?)
    }

    async fn revive_with_membership(
        &self,
        tx: &mut Transaction,
        role: Role,
        membership: &Membership,
        left_at: DateTime<Utc>,
    ) -> Result<(), TableError> {
        Ok(revive_with_membership(tx, role, membership, left_at).await?)
    }

    async fn move_rows_to_community(
        &self,
        tx: &mut Transaction,
        from_community_id: Uuid,
        to: &Membership,
    ) -> Result<(), TableError> {
        Ok(app_posts_communities::maintenance::move_rows_to_community(
            tx,
            from_community_id,
            to.community_id,
            to.id,
        )
        .await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::memberships::MembershipError;
    use crate::app::posts::PostError;
    use crate::app::{communities, memberships, posts, users};
    use crate::database::{Database, with_each_database};
    use crate::services::web_server::InProcess;
    use crate::sync::tests::{device_of, user_with_token};
    use crate::sync::{PullRequest, Report, pull, run};

    async fn sync(device: &Deployment, server: &Deployment) -> Report {
        run(device, &InProcess { server }).await.expect("a sync")
    }

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
    async fn posts_communities_a_post_is_seen_in_the_communities_its_owner_chose() {
        with_each_database(|database| async move {
            let deployment = server(database);
            let alice = users::create(&deployment, "alice@example.com", "Alice")
                .await
                .expect("a user");
            let bob = users::create(&deployment, "bob@example.com", "Bob")
                .await
                .expect("a user");
            let carol = users::create(&deployment, "carol@example.com", "Carol")
                .await
                .expect("a user");
            let dave = users::create(&deployment, "dave@example.com", "Dave")
                .await
                .expect("a user");
            let gardening = communities::create(&deployment, &alice, "Gardening")
                .await
                .expect("a community");
            let cooking = communities::create(&deployment, &alice, "Cooking")
                .await
                .expect("a community");
            let woodwork = communities::create(&deployment, &dave, "Woodwork")
                .await
                .expect("a community");
            memberships::create(&deployment, &bob, gardening.id)
                .await
                .expect("joined");
            memberships::create(&deployment, &carol, cooking.id)
                .await
                .expect("joined");
            let shared = posts::create(&deployment, &alice, "shared", &[gardening.id, cooking.id])
                .await
                .expect("a post");
            posts::create(&deployment, &alice, "private", &[])
                .await
                .expect("a post");
            posts::create(&deployment, &dave, "dave's", &[woodwork.id])
                .await
                .expect("a post");

            assert_eq!(bodies(&deployment, &alice).await, vec!["private", "shared"]);
            assert_eq!(bodies(&deployment, &bob).await, vec!["shared"]);
            assert_eq!(bodies(&deployment, &carol).await, vec!["shared"]);
            assert_eq!(bodies(&deployment, &dave).await, vec!["dave's"]);
            let error = posts::get(&deployment, &dave, shared.id)
                .await
                .expect_err("not in any of dave's communities");
            assert!(matches!(error, PostError::NotFound { .. }));
            let listed = posts::get(&deployment, &bob, shared.id)
                .await
                .expect("in gardening");
            assert_eq!(listed.owner.name, "Alice");
            let names: Vec<&str> = listed
                .communities
                .iter()
                .map(|community| community.name.as_str())
                .collect();
            assert_eq!(names, vec!["Gardening", "Cooking"]);
        })
        .await;
    }

    #[tokio::test]
    async fn posts_communities_add_needs_the_owner_in_the_community() {
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

            let error = posts::create(&deployment, &bob, "bob's", &[gardening.id])
                .await
                .expect_err("not a member");
            assert!(matches!(
                error,
                PostError::Community(PostCommunityError::NotAMember { .. })
            ));
            assert!(
                bodies(&deployment, &bob).await.is_empty(),
                "nothing written"
            );

            let bobs = posts::create(&deployment, &bob, "bob's", &[])
                .await
                .expect("a post");
            let error = add(&deployment, &alice, bobs.id, gardening.id)
                .await
                .expect_err("not hers");
            assert!(matches!(error, PostCommunityError::NotOwner));
            let error = remove(&deployment, &bob, bobs.id, gardening.id)
                .await
                .expect_err("not in it");
            assert!(matches!(error, PostCommunityError::NotFound { .. }));
        })
        .await;
    }

    #[tokio::test]
    async fn posts_communities_removing_from_one_community_leaves_the_others_and_adding_back_revives()
     {
        with_each_database(|database| async move {
            let deployment = server(database);
            let alice = users::create(&deployment, "alice@example.com", "Alice")
                .await
                .expect("a user");
            let bob = users::create(&deployment, "bob@example.com", "Bob")
                .await
                .expect("a user");
            let carol = users::create(&deployment, "carol@example.com", "Carol")
                .await
                .expect("a user");
            let gardening = communities::create(&deployment, &alice, "Gardening")
                .await
                .expect("a community");
            let cooking = communities::create(&deployment, &alice, "Cooking")
                .await
                .expect("a community");
            memberships::create(&deployment, &bob, gardening.id)
                .await
                .expect("joined");
            memberships::create(&deployment, &carol, cooking.id)
                .await
                .expect("joined");
            let post = posts::create(&deployment, &alice, "shared", &[gardening.id, cooking.id])
                .await
                .expect("a post");
            let mut tx = deployment.begin().await.expect("a transaction");
            let first = app_posts_communities::replication::get_by_post_and_community(
                &mut tx,
                post.id,
                gardening.id,
            )
            .await
            .expect("the row")
            .expect("there");
            tx.commit().await.expect("committed");

            posts::update(&deployment, &alice, post.id, "shared", &[cooking.id])
                .await
                .expect("taken out of gardening");
            assert!(bodies(&deployment, &bob).await.is_empty());
            assert_eq!(bodies(&deployment, &carol).await, vec!["shared"]);

            posts::update(
                &deployment,
                &alice,
                post.id,
                "shared",
                &[cooking.id, gardening.id],
            )
            .await
            .expect("put back");
            assert_eq!(bodies(&deployment, &bob).await, vec!["shared"]);
            let mut tx = deployment.begin().await.expect("a transaction");
            let back = app_posts_communities::replication::get_by_post_and_community(
                &mut tx,
                post.id,
                gardening.id,
            )
            .await
            .expect("the row")
            .expect("there");
            assert_eq!(back.id, first.id, "the same row");
            assert_eq!(back.version, 3);
            assert!(back.deleted_at.is_none());
        })
        .await;
    }

    #[tokio::test]
    async fn posts_communities_deleting_the_post_deletes_its_rows_with_it() {
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
            memberships::create(&deployment, &bob, gardening.id)
                .await
                .expect("joined");
            let post = posts::create(&deployment, &alice, "going", &[gardening.id])
                .await
                .expect("a post");
            assert_eq!(bodies(&deployment, &bob).await, vec!["going"]);

            posts::delete(&deployment, &alice, post.id)
                .await
                .expect("deleted");
            assert!(bodies(&deployment, &bob).await.is_empty());
            let mut tx = deployment.begin().await.expect("a transaction");
            let deleted = crate::data::app_posts::replication::get(&mut tx, post.id)
                .await
                .expect("the post")
                .expect("kept");
            let row = app_posts_communities::replication::get_by_post_and_community(
                &mut tx,
                post.id,
                gardening.id,
            )
            .await
            .expect("the row")
            .expect("kept");
            assert_eq!(row.deleted_at, deleted.deleted_at);
            assert_eq!(row.deleted_by_id, Some(alice.id));
            assert_eq!(row.version, 2);
        })
        .await;
    }

    async fn community_names(deployment: &Deployment, user: &User, post_id: Uuid) -> Vec<String> {
        posts::get(deployment, user, post_id)
            .await
            .expect("the post")
            .communities
            .into_iter()
            .map(|community| community.name)
            .collect()
    }

    #[tokio::test]
    async fn posts_communities_a_post_taken_out_leaves_the_other_members_device_and_comes_back() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let (alice, _) = user_with_token(&server, "alice@example.com").await;
            let (bob, bob_token) = user_with_token(&server, "bob@example.com").await;
            let gardening = communities::create(&server, &alice, "Gardening")
                .await
                .expect("a community");
            memberships::create(&server, &bob, gardening.id)
                .await
                .expect("joined");
            let post = posts::create(&server, &alice, "roses", &[gardening.id])
                .await
                .expect("a post");
            let b = device_of(&bob, &bob_token).await;
            sync(&b, &server).await;
            assert_eq!(bodies(&b, &bob).await, vec!["roses"]);

            remove(&server, &alice, post.id, gardening.id)
                .await
                .expect("removed");
            let report = sync(&b, &server).await;
            assert_eq!(report.pruned, 1);
            assert!(bodies(&b, &bob).await.is_empty());

            add(&server, &alice, post.id, gardening.id)
                .await
                .expect("added back");
            let report = sync(&b, &server).await;
            assert_eq!(report.pruned, 0);
            assert_eq!(bodies(&b, &bob).await, vec!["roses"]);
        })
        .await;
    }

    #[tokio::test]
    async fn posts_communities_adding_to_a_community_left_meanwhile_is_refused_and_dropped() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let (bob, bob_token) = user_with_token(&server, "bob@example.com").await;
            let gardening = communities::create(&server, &bob, "Gardening")
                .await
                .expect("a community");
            let b = device_of(&bob, &bob_token).await;
            sync(&b, &server).await;
            memberships::delete(&server, &bob, gardening.id)
                .await
                .expect("left on the web");

            let post = posts::create(&b, &bob, "late", &[gardening.id])
                .await
                .expect("a post, offline");
            assert_eq!(community_names(&b, &bob, post.id).await, vec!["Gardening"]);
            let report = sync(&b, &server).await;
            assert_eq!(report.pushed, 1, "the post");
            assert_eq!(report.rejected, 1, "its community row");
            assert_eq!(report.pending, 0);
            assert_eq!(bodies(&b, &bob).await, vec!["late"]);
            assert!(community_names(&b, &bob, post.id).await.is_empty());
            assert!(community_names(&server, &bob, post.id).await.is_empty());
            assert!(
                memberships::list_mine(&b, &bob)
                    .await
                    .expect("memberships")
                    .is_empty()
            );
        })
        .await;
    }

    #[tokio::test]
    async fn posts_communities_a_post_taken_out_before_a_device_joined_is_never_sent_to_it() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let (alice, _) = user_with_token(&server, "alice@example.com").await;
            let (bob, bobs_token) = user_with_token(&server, "bob@example.com").await;
            let gardening = communities::create(&server, &alice, "Gardening")
                .await
                .expect("a community");
            let cooking = communities::create(&server, &alice, "Cooking")
                .await
                .expect("a community");
            let post = posts::create(&server, &alice, "roses", &[gardening.id, cooking.id])
                .await
                .expect("a post");
            remove(&server, &alice, post.id, gardening.id)
                .await
                .expect("taken out of gardening");
            memberships::create(&server, &bob, gardening.id)
                .await
                .expect("joined after");

            let page = pull(&server, &bob, PullRequest::default())
                .await
                .expect("a page");
            assert!(
                page.rows.iter().all(|row| row.table != app_posts::TABLE),
                "the post is in cooking only, which bob is not in"
            );
            let device = device_of(&bob, &bobs_token).await;
            run(&device, &InProcess { server: &server })
                .await
                .expect("a sync");
            let mut tx = device.begin().await.expect("a transaction");
            assert!(
                posts::get_in(&mut tx, post.id)
                    .await
                    .expect("read")
                    .is_none()
            );
            tx.commit().await.expect("committed");
        })
        .await;
    }

    #[tokio::test]
    async fn posts_communities_leaving_takes_own_posts_out_and_rejoining_brings_back_what_left() {
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
            memberships::create(&deployment, &bob, gardening.id)
                .await
                .expect("joined");
            let roses = posts::create(&deployment, &alice, "roses", &[gardening.id])
                .await
                .expect("a post");
            let weeds = posts::create(&deployment, &alice, "weeds", &[gardening.id])
                .await
                .expect("a post");
            let bobs = posts::create(&deployment, &bob, "bob's", &[gardening.id])
                .await
                .expect("a post");
            remove(&deployment, &alice, weeds.id, gardening.id)
                .await
                .expect("removed on purpose");
            assert_eq!(bodies(&deployment, &bob).await, vec!["bob's", "roses"]);

            let membership = memberships::list_mine(&deployment, &alice)
                .await
                .expect("memberships")
                .remove(0);
            memberships::delete(&deployment, &alice, gardening.id)
                .await
                .expect("left");
            assert!(
                memberships::list_mine(&deployment, &alice)
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
            let left = memberships::get_in(&mut tx, membership.id)
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
            let error = memberships::delete(&deployment, &alice, gardening.id)
                .await
                .expect_err("already left");
            assert!(matches!(error, MembershipError::NotFound { .. }));

            let rejoined = memberships::create(&deployment, &alice, gardening.id)
                .await
                .expect("rejoined");
            assert_eq!(rejoined.id, membership.id, "the same row");
            assert_eq!(rejoined.version, 3);
            assert!(rejoined.deleted_at.is_none());
            assert_eq!(bodies(&deployment, &bob).await, vec!["bob's", "roses"]);
            assert_eq!(
                memberships::create(&deployment, &alice, gardening.id)
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
    async fn posts_communities_rejoining_leaves_a_post_deleted_meanwhile_deleted() {
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
            memberships::create(&deployment, &bob, gardening.id)
                .await
                .expect("joined");
            let roses = posts::create(&deployment, &alice, "roses", &[gardening.id])
                .await
                .expect("a post");
            memberships::delete(&deployment, &alice, gardening.id)
                .await
                .expect("left");
            posts::delete(&deployment, &alice, roses.id)
                .await
                .expect("deleted");

            memberships::create(&deployment, &alice, gardening.id)
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
}
