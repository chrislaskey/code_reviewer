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

use crate::data::app_posts_communities;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;
use uuid::Uuid;

use crate::app::events::{self, Answer, AnswerStatus, Conflicts, Event, EventKind};
use crate::app::memberships::{self, Membership, id_field};
use crate::app::sync::{Cursor, TableRow};
use crate::app::users::User;
use crate::data::app_posts::Post;
use crate::database::Transaction;
use crate::deployment::{Deployment, Role, now};
use crate::helpers::json;

/// A row accepted by this resource's event or replication workflow.
/// Only this module can construct an accepted write.
pub(crate) struct AcceptedPostCommunity<'a>(&'a PostCommunity);

impl AcceptedPostCommunity<'_> {
    /// Borrows the accepted row for persistence.
    pub(crate) fn row(&self) -> &PostCommunity {
        self.0
    }
}

pub const TABLE: &str = "app_posts_communities";
const CREATE_FIELDS: &[&str] = &["post_id", "community_id", "users_communities_id"];
const UPDATE_FIELDS: &[&str] = &["deleted_at"];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PostCommunity {
    pub id: Uuid,
    pub post_id: Uuid,
    pub community_id: Uuid,

    /// The owner's membership the post was added under.
    pub users_communities_id: Uuid,
    pub inserted_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub version: i64,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub deleted_by_id: Option<Uuid>,
}

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
    Record(#[from] crate::app::events::RecordError),
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
pub(crate) async fn delete_with_membership(
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
pub(crate) async fn revive_with_membership(
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
    if events::has_pending(tx, TABLE, row.id).await? {
        return Ok(false);
    }
    if crate::data::app_posts::replication::get(tx, row.post_id)
        .await?
        .is_none()
    {
        return Ok(false);
    }
    app_posts_communities::replication::upsert(&mut *tx, AcceptedPostCommunity(&row)).await?;
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
    after: &Cursor,
    limit: usize,
) -> Result<Vec<PostCommunity>, PostCommunityError> {
    Ok(app_posts_communities::replication::list_synced_after(tx, user.id, after, limit).await?)
}

/// A page of the community's live rows after the cursor: a backfill.
pub(crate) async fn list_live_of_community_after(
    tx: &mut Transaction,
    community_id: Uuid,
    after: &Cursor,
    limit: usize,
) -> Result<Vec<PostCommunity>, PostCommunityError> {
    Ok(
        app_posts_communities::replication::list_live_of_community_after(
            tx,
            community_id,
            after,
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
        && let Some(recorded) = events::get(tx, event.id).await?
    {
        let row = get_in(tx, event.row_id).await?;
        let server = row.as_ref().map(table_row).transpose()?;
        return Ok(Applied {
            answer: recorded.answer(server),
            row,
        });
    }

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
                        deleted_at: None,
                        deleted_by_id: None,
                    };
                    app_posts_communities::create(&mut *tx, AcceptedPostCommunity(&row)).await?;
                    row
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
    app_posts_communities::update(&mut *tx, AcceptedPostCommunity(row)).await?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::posts::PostError;
    use crate::app::{communities, memberships, posts, users};
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
            memberships::join(&deployment, &bob, gardening.id)
                .await
                .expect("joined");
            memberships::join(&deployment, &carol, cooking.id)
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
            memberships::join(&deployment, &bob, gardening.id)
                .await
                .expect("joined");
            memberships::join(&deployment, &carol, cooking.id)
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
            memberships::join(&deployment, &bob, gardening.id)
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
}
