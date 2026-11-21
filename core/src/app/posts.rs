//! Posts: the first resource. A post is some text with an owner, written,
//! edited and deleted by its owner from any deployment, in the
//! communities the owner chose, see `posts_communities`, and seen by its
//! owner and the members of those communities. Called through the
//! module, `posts::list`, as the standard library's `fs::read` is.
//!
//! Every write is an event applied by `apply_event`, on the server at
//! once and on a device at once too, where it is queued and the server
//! answers it on the next push with the same function, see `events`. The
//! user acting owns what they create, may change only what they own,
//! and is recorded as `deleted_by_id` on what they delete, which takes
//! the post's community rows with it.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;
use uuid::Uuid;

use crate::app::communities;
use crate::app::events::{self, Answer, AnswerStatus};
use crate::app::posts_communities::{self, PostCommunityError};
use crate::app::sync::TableRow;
use crate::app::users;
use crate::data::app_communities::Community;
use crate::data::app_events::{self, Conflicts, Event, EventKind, Fields};
use crate::data::app_posts::{self, AcceptedPost, Post, PostInvalid, TABLE};
use crate::data::app_sync_state::Cursor;
use crate::data::app_users::User;
use crate::database::Transaction;
use crate::deployment::{Deployment, Role, now};
use crate::helpers::json;

/// The fields an event may set when it creates a post.
const CREATE_FIELDS: &[&str] = &["body"];
/// The fields an event may change on a post.
const UPDATE_FIELDS: &[&str] = &["body", "deleted_at"];

/// A post as the pages show it: with who wrote it and where it is.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PostWithOwnerAndCommunities {
    pub post: Post,
    pub owner: User,
    pub communities: Vec<Community>,
}

#[derive(Debug, Error)]
pub enum PostError {
    #[error("post {id} not found")]
    NotFound { id: Uuid },
    #[error(transparent)]
    Validation(#[from] PostInvalid),
    #[error("only the owner can change a post")]
    NotOwner,
    #[error("{0}")]
    Invalid(String),
    #[error("the change was refused: {0}")]
    Rejected(String),
    #[error(transparent)]
    Community(#[from] PostCommunityError),
    #[error(transparent)]
    Record(#[from] crate::app::events::RecordError),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl From<serde_json::Error> for PostError {
    fn from(error: serde_json::Error) -> PostError {
        PostError::Invalid(error.to_string())
    }
}

/// Every post the user sees, not deleted, newest first, each with its
/// owner and communities.
pub async fn list(
    deployment: &Deployment,
    user: &User,
) -> Result<Vec<PostWithOwnerAndCommunities>, PostError> {
    let mut tx = deployment.begin().await?;
    let posts = app_posts::list(&mut tx, app_posts::Filter::VisibleTo(user.id)).await?;
    let listed = with_owners_and_communities(&mut tx, posts).await?;
    tx.commit().await?;
    Ok(listed)
}

/// One post the user sees, by its id, unless deleted, with its owner and
/// communities.
pub async fn get(
    deployment: &Deployment,
    user: &User,
    id: Uuid,
) -> Result<PostWithOwnerAndCommunities, PostError> {
    let mut tx = deployment.begin().await?;
    let post = get_live(&mut tx, user, id).await?;
    let mut listed = with_owners_and_communities(&mut tx, vec![post]).await?;
    tx.commit().await?;
    listed.pop().ok_or(PostError::NotFound { id })
}

/// Writes a new post of the user's, with an id and the time from this
/// deployment, in the communities, which the user must be in.
pub async fn create(
    deployment: &Deployment,
    user: &User,
    body: &str,
    community_ids: &[Uuid],
) -> Result<Post, PostError> {
    let body = app_posts::checked_body(body)?;
    let event = Event::create(
        TABLE,
        Uuid::now_v7(),
        json::object_fields(json!({ "body": body })),
        now(),
    );
    let mut tx = deployment.begin().await?;
    let post = apply_in(&mut tx, deployment.role, user, &event)
        .await?
        .post()?;
    posts_communities::set_in(&mut tx, deployment.role, user, post.id, community_ids).await?;
    tx.commit().await?;
    Ok(post)
}

/// Writes a new body over the user's post, with the time from this
/// deployment, and makes its communities exactly these.
pub async fn update(
    deployment: &Deployment,
    user: &User,
    id: Uuid,
    body: &str,
    community_ids: &[Uuid],
) -> Result<Post, PostError> {
    let body = app_posts::checked_body(body)?;
    let mut tx = deployment.begin().await?;
    let post = get_own(&mut tx, user, id).await?;
    let post = if post.body == body {
        post
    } else {
        let event = Event::update(
            TABLE,
            id,
            post.version,
            json::object_fields(json!({ "body": post.body })),
            json::object_fields(json!({ "body": body })),
            now(),
        );
        apply_in(&mut tx, deployment.role, user, &event)
            .await?
            .post()?
    };
    posts_communities::set_in(&mut tx, deployment.role, user, post.id, community_ids).await?;
    tx.commit().await?;
    Ok(post)
}

/// Deletes the user's post: a soft delete, so it syncs like any edit.
pub async fn delete(deployment: &Deployment, user: &User, id: Uuid) -> Result<(), PostError> {
    let mut tx = deployment.begin().await?;
    let post = get_own(&mut tx, user, id).await?;
    let event = Event::update(
        TABLE,
        id,
        post.version,
        json::object_fields(json!({ "deleted_at": null })),
        json::object_fields(json!({ "deleted_at": now() })),
        now(),
    );
    let applied = apply_in(&mut tx, deployment.role, user, &event).await?;
    tx.commit().await?;
    applied.post().map(|_| ())
}

/// Applies an event of the user's to its post and answers it: what the
/// server does with a pushed event, and what a local write does on any
/// deployment.
pub async fn apply_event(
    deployment: &Deployment,
    user: &User,
    event: &Event,
) -> Result<Answer, PostError> {
    let mut tx = deployment.begin().await?;
    let applied = apply_in(&mut tx, deployment.role, user, event).await?;
    tx.commit().await?;
    Ok(applied.answer)
}

/// The post as it travels on the wire.
pub fn table_row(post: &Post) -> Result<TableRow, PostError> {
    Ok(TableRow {
        table: TABLE.to_string(),
        row: serde_json::to_value(post)?,
    })
}

/// Writes the server's row over the device's, unless an event of the
/// device's for it is still pending; true when it was written.
pub(crate) async fn apply_server_row(tx: &mut Transaction, row: &Value) -> Result<bool, PostError> {
    let post: Post = serde_json::from_value(row.clone())?;
    if app_events::has_pending(tx, TABLE, post.id).await? {
        return Ok(false);
    }
    app_posts::replication::upsert(tx, &post).await?;
    Ok(true)
}

/// Removes a device's post whose create the server refused.
pub(crate) async fn delete_row(tx: &mut Transaction, id: Uuid) -> Result<(), PostError> {
    Ok(app_posts::maintenance::delete(tx, id).await?)
}

/// Removes every post the user no longer sees from a device, after a
/// pull: not theirs, and in no community they are in. How many went.
pub(crate) async fn prune(tx: &mut Transaction, user: &User) -> Result<u64, PostError> {
    Ok(app_posts::maintenance::prune(tx, user.id).await?)
}

/// The posts with their owners and live communities, in the order given.
async fn with_owners_and_communities(
    tx: &mut Transaction,
    posts: Vec<Post>,
) -> Result<Vec<PostWithOwnerAndCommunities>, PostError> {
    let user_ids: Vec<Uuid> = posts.iter().map(|post| post.user_id).collect();
    let owners = users::list_by_ids(tx, &user_ids)
        .await
        .map_err(|error| PostError::Invalid(error.to_string()))?;
    let post_ids: Vec<Uuid> = posts.iter().map(|post| post.id).collect();
    let rows = posts_communities::list_live_of_posts(tx, &post_ids).await?;
    let community_ids: Vec<Uuid> = rows.iter().map(|row| row.community_id).collect();
    let communities = communities::list_by_ids(tx, &community_ids)
        .await
        .map_err(|error| PostError::Invalid(error.to_string()))?;
    posts
        .into_iter()
        .map(|post| {
            let owner = owners
                .iter()
                .find(|owner| owner.id == post.user_id)
                .cloned()
                .ok_or(PostError::Invalid(format!("post {} has no owner", post.id)))?;
            let communities = rows
                .iter()
                .filter(|row| row.post_id == post.id)
                .filter_map(|row| {
                    communities
                        .iter()
                        .find(|community| community.id == row.community_id)
                })
                .filter(|community| community.deleted_at.is_none())
                .cloned()
                .collect();
            Ok(PostWithOwnerAndCommunities {
                post,
                owner,
                communities,
            })
        })
        .collect()
}

/// A page of the rows the server wrote after the cursor that the user
/// sees, deleted or not.
pub(crate) async fn list_synced_after(
    tx: &mut Transaction,
    user: &User,
    after: &Cursor,
    limit: usize,
) -> Result<Vec<Post>, PostError> {
    Ok(app_posts::replication::list(tx, user.id, after, limit).await?)
}

/// The row by its id, deleted or not, whoever sees it.
pub(crate) async fn get_in(tx: &mut Transaction, id: Uuid) -> Result<Option<Post>, PostError> {
    Ok(app_posts::replication::get(tx, id).await?)
}

/// The live post, if the user sees it.
async fn get_live(tx: &mut Transaction, user: &User, id: Uuid) -> Result<Post, PostError> {
    match app_posts::get(tx, user.id, id).await? {
        Some(post) if post.deleted_at.is_none() => Ok(post),
        _ => Err(PostError::NotFound { id }),
    }
}

/// The live post, if the user owns it.
async fn get_own(tx: &mut Transaction, user: &User, id: Uuid) -> Result<Post, PostError> {
    let post = get_live(tx, user, id).await?;
    if post.user_id != user.id {
        return Err(PostError::NotOwner);
    }
    Ok(post)
}

/// An event applied: the answer, and the post as it now stands.
pub(crate) struct Applied {
    pub answer: Answer,
    pub post: Option<Post>,
}

impl Applied {
    fn post(self) -> Result<Post, PostError> {
        match (self.answer.status, self.post) {
            (AnswerStatus::Rejected, _) => Err(PostError::Rejected(
                self.answer.reason.unwrap_or_else(|| "rejected".to_string()),
            )),
            (_, Some(post)) => Ok(post),
            (_, None) => Err(PostError::NotFound { id: self.answer.id }),
        }
    }
}

/// Applies the event in the transaction: checks the fields it changes
/// and that the user owns the post, merges it into the row, writes the
/// row, and then the role's part, the record of the answer on the server
/// or the queue on a device.
pub(crate) async fn apply_in(
    tx: &mut Transaction,
    role: Role,
    user: &User,
    event: &Event,
) -> Result<Applied, PostError> {
    if role == Role::Server
        && let Some(recorded) = app_events::get(tx, event.id).await?
    {
        let post = get_in(tx, event.row_id).await?;
        let server = post.as_ref().map(table_row).transpose()?;
        return Ok(Applied {
            answer: recorded.answer(server),
            post,
        });
    }

    let allowed = match event.kind {
        EventKind::Create => CREATE_FIELDS,
        EventKind::Update => UPDATE_FIELDS,
    };
    if let Some(reason) = events::not_editable(&event.changes, allowed) {
        let post = get_in(tx, event.row_id).await?;
        return rejected(event, reason, post);
    }

    let now = now();
    let post = match event.kind {
        EventKind::Create => {
            if let Some(existing) = get_in(tx, event.row_id).await? {
                return rejected(event, "exists", Some(existing));
            }
            let (version, last_synced_at) = match role {
                Role::Server => (1, Some(now)),
                Role::Device => (0, None),
            };
            let blank = Post {
                id: event.row_id,
                user_id: user.id,
                body: String::new(),
                inserted_at: event.row_inserted_at(now),
                updated_at: now,
                version,
                last_synced_at,
                deleted_at: None,
                deleted_by_id: None,
            };
            let accepted = blank
                .with_fields(&event.changes)
                .and_then(|post| Ok(AcceptedPost::new(post)?));
            let post = match accepted {
                Ok(post) => post,
                Err(error) => return rejected(event, format!("invalid: {error}"), None),
            };
            app_posts::create(tx, &post).await?;
            finish(tx, role, user, event, Conflicts::new(), post.into_row()).await?
        }
        EventKind::Update => {
            let Some(post) = get_in(tx, event.row_id).await? else {
                return rejected(event, "unknown_post", None);
            };
            if post.user_id != user.id {
                return rejected(event, "not_owner", Some(post));
            }
            let (changes, conflicts) = events::merge(
                &post.fields(UPDATE_FIELDS)?,
                post.version,
                event.base_version,
                &event.base,
                &event.changes,
            );
            let post = if changes.is_empty() {
                post
            } else {
                let mut updated = match post.with_fields(&changes) {
                    Ok(updated) => updated,
                    Err(error) => return rejected(event, format!("invalid: {error}"), Some(post)),
                };
                updated.updated_at = now;
                if changes.contains_key("deleted_at") {
                    updated.deleted_by_id = updated.deleted_at.map(|_| user.id);
                }
                if role == Role::Server {
                    updated.version += 1;
                    updated.last_synced_at = Some(now);
                }
                let updated = match AcceptedPost::new(updated) {
                    Ok(updated) => updated,
                    Err(error) => return rejected(event, format!("invalid: {error}"), Some(post)),
                };
                app_posts::update(tx, &updated).await?;
                let updated = updated.into_row();
                if changes.contains_key("deleted_at") && updated.deleted_at.is_some() {
                    posts_communities::delete_with_post(tx, role, &updated).await?;
                }
                updated
            };
            finish(tx, role, user, event, conflicts, post).await?
        }
    };
    Ok(post)
}

/// Answers an applied event and does the role's part.
async fn finish(
    tx: &mut Transaction,
    role: Role,
    user: &User,
    event: &Event,
    conflicts: Conflicts,
    post: Post,
) -> Result<Applied, PostError> {
    let answer = Answer::applied(event.id, post.version, conflicts, table_row(&post)?);
    match role {
        Role::Server => events::record(tx, user.id, event, &answer).await?,
        Role::Device => events::queue(tx, user.id, event).await?,
    }
    Ok(Applied {
        answer,
        post: Some(post),
    })
}

fn rejected(
    event: &Event,
    reason: impl Into<String>,
    post: Option<Post>,
) -> Result<Applied, PostError> {
    let server = post.as_ref().map(table_row).transpose()?;
    Ok(Applied {
        answer: Answer::rejected(event.id, reason, server),
        post,
    })
}

impl Post {
    /// The named fields as they travel on the wire.
    pub(crate) fn fields(&self, names: &[&str]) -> Result<Fields, PostError> {
        let mut all = json::object_fields(serde_json::to_value(self)?);
        all.retain(|name, _| names.contains(&name.as_str()));
        Ok(all)
    }

    /// The post with the fields written over it, not yet checked.
    fn with_fields(&self, changes: &Fields) -> Result<Post, PostError> {
        let mut all = json::object_fields(serde_json::to_value(self)?);
        for (name, value) in changes {
            all.insert(name.clone(), value.clone());
        }
        Ok(serde_json::from_value(Value::Object(all))?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::users;
    use crate::data::app_events::EventStatus;
    use crate::database::{Database, with_each_database};

    fn server(database: Database) -> Deployment {
        Deployment::new(database, Role::Server)
    }

    /// A user registered on the server.
    pub(crate) async fn user(deployment: &Deployment, email: &str) -> User {
        users::create(deployment, email, email.split('@').next().unwrap_or(email))
            .await
            .expect("a user")
    }

    async fn events_with_status(deployment: &Deployment, status: EventStatus) -> i64 {
        let mut tx = deployment.begin().await.expect("a transaction");
        app_events::count(&mut tx, status).await.expect("a count")
    }

    #[tokio::test]
    async fn posts_create_with_unknown_community_rolls_back_row_and_event() {
        with_each_database(|database| async move {
            let deployment = server(database);
            let alice = user(&deployment, "alice@example.com").await;
            let before = events_with_status(&deployment, EventStatus::Applied).await;
            assert!(
                create(&deployment, &alice, "rolled back", &[Uuid::now_v7()])
                    .await
                    .is_err()
            );
            assert!(list(&deployment, &alice).await.expect("posts").is_empty());
            assert_eq!(
                events_with_status(&deployment, EventStatus::Applied).await,
                before
            );
        })
        .await;
    }

    #[tokio::test]
    async fn posts_records_get_excludes_inaccessible_and_deleted_rows() {
        with_each_database(|database| async move {
            let deployment = server(database);
            let alice = user(&deployment, "alice@example.com").await;
            let bob = user(&deployment, "bob@example.com").await;
            let post = create(&deployment, &alice, "private", &[])
                .await
                .expect("post");
            let mut tx = deployment.begin().await.expect("transaction");
            assert!(
                app_posts::get(&mut tx, bob.id, post.id)
                    .await
                    .expect("read")
                    .is_none()
            );
            assert!(
                app_posts::get(&mut tx, alice.id, post.id)
                    .await
                    .expect("read")
                    .is_some()
            );
            tx.commit().await.expect("commit");
            delete(&deployment, &alice, post.id).await.expect("delete");
            let mut tx = deployment.begin().await.expect("transaction");
            assert!(
                app_posts::get(&mut tx, alice.id, post.id)
                    .await
                    .expect("read")
                    .is_none()
            );
            assert!(
                app_posts::replication::get(&mut tx, post.id)
                    .await
                    .expect("tombstone")
                    .expect("row")
                    .deleted_at
                    .is_some()
            );
        })
        .await;
    }

    #[tokio::test]
    async fn posts_list_newest_first() {
        with_each_database(|database| async move {
            let deployment = server(database);
            let alice = user(&deployment, "alice@example.com").await;
            let first = create(&deployment, &alice, "first", &[])
                .await
                .expect("a post");
            let second = create(&deployment, &alice, "second", &[])
                .await
                .expect("a post");

            let posts = list(&deployment, &alice).await.expect("the posts");
            let listed: Vec<&Post> = posts.iter().map(|listed| &listed.post).collect();
            assert_eq!(listed, vec![&second, &first]);
            assert_eq!(posts[0].owner, alice);
            assert!(posts[0].communities.is_empty());
        })
        .await;
    }

    #[tokio::test]
    async fn posts_list_and_get_show_a_user_their_own_posts_only() {
        with_each_database(|database| async move {
            let deployment = server(database);
            let alice = user(&deployment, "alice@example.com").await;
            let bob = user(&deployment, "bob@example.com").await;
            let post = create(&deployment, &alice, "alice's", &[])
                .await
                .expect("a post");
            assert_eq!(post.user_id, alice.id);

            assert!(list(&deployment, &bob).await.expect("the posts").is_empty());
            let error = get(&deployment, &bob, post.id)
                .await
                .expect_err("not bob's");
            assert!(matches!(error, PostError::NotFound { .. }));
            assert_eq!(
                get(&deployment, &alice, post.id)
                    .await
                    .expect("the post")
                    .post,
                post
            );
        })
        .await;
    }

    #[tokio::test]
    async fn posts_get_missing() {
        with_each_database(|database| async move {
            let deployment = server(database);
            let alice = user(&deployment, "alice@example.com").await;
            let id = Uuid::now_v7();

            let error = get(&deployment, &alice, id)
                .await
                .expect_err("no such post");
            assert!(matches!(error, PostError::NotFound { id: missing } if missing == id));
        })
        .await;
    }

    #[tokio::test]
    async fn posts_create_trims_and_refuses_blank() {
        with_each_database(|database| async move {
            let deployment = server(database);
            let alice = user(&deployment, "alice@example.com").await;
            let post = create(&deployment, &alice, "  spaced  ", &[])
                .await
                .expect("a post");
            assert_eq!(post.body, "spaced");

            let error = create(&deployment, &alice, " \n ", &[])
                .await
                .expect_err("blank");
            assert!(matches!(
                error,
                PostError::Validation(PostInvalid::BlankBody)
            ));
            assert_eq!(list(&deployment, &alice).await.expect("the posts").len(), 1);
        })
        .await;
    }

    #[tokio::test]
    async fn posts_create_and_update_refuse_a_body_over_the_maximum_length() {
        with_each_database(|database| async move {
            let deployment = server(database);
            let alice = user(&deployment, "alice@example.com").await;
            let longest = "é".repeat(app_posts::MAX_BODY_LENGTH);
            let post = create(&deployment, &alice, &longest, &[])
                .await
                .expect("the maximum, counted in characters");
            let too_long = format!("{longest}é");
            let error = create(&deployment, &alice, &too_long, &[])
                .await
                .expect_err("one more");
            assert!(matches!(
                error,
                PostError::Validation(PostInvalid::BodyTooLong)
            ));
            let error = update(&deployment, &alice, post.id, &too_long, &[])
                .await
                .expect_err("one more");
            assert!(matches!(
                error,
                PostError::Validation(PostInvalid::BodyTooLong)
            ));

            let pushed = Event::create(
                TABLE,
                Uuid::now_v7(),
                json::object_fields(json!({ "body": too_long })),
                now(),
            );
            let answer = apply_event(&deployment, &alice, &pushed)
                .await
                .expect("answered");
            assert_eq!(answer.status, AnswerStatus::Rejected, "a pushed one too");
            assert_eq!(list(&deployment, &alice).await.expect("the posts").len(), 1);
        })
        .await;
    }

    #[tokio::test]
    async fn posts_create_on_the_server_is_version_one_and_logged_with_its_user() {
        with_each_database(|database| async move {
            let deployment = server(database);
            let alice = user(&deployment, "alice@example.com").await;
            let post = create(&deployment, &alice, "a post", &[])
                .await
                .expect("a post");
            assert_eq!(post.version, 1);
            assert_eq!(post.last_synced_at, Some(post.updated_at));
            assert_eq!(
                events_with_status(&deployment, EventStatus::Applied).await,
                1
            );
            assert_eq!(
                events_with_status(&deployment, EventStatus::Pending).await,
                0
            );
            let mut tx = deployment.begin().await.expect("a transaction");
            let logged = app_events::conflicts(&mut tx).await.expect("none");
            assert!(logged.is_empty());
        })
        .await;
    }

    #[tokio::test]
    async fn posts_create_on_a_device_is_version_zero_and_queued() {
        let alice = users::tests::alice();
        let deployment = users::tests::device_of(&alice, "a token").await;
        let post = create(&deployment, &alice, "a post", &[])
            .await
            .expect("a post");
        assert_eq!(post.version, 0);
        assert_eq!(post.last_synced_at, None);
        assert_eq!(post.user_id, alice.id);
        assert_eq!(
            events_with_status(&deployment, EventStatus::Pending).await,
            1
        );
        let mut tx = deployment.begin().await.expect("a transaction");
        let pending = app_events::pending(&mut tx).await.expect("the queue");
        assert_eq!(pending[0].user_id, alice.id);
    }

    #[tokio::test]
    async fn posts_update() {
        with_each_database(|database| async move {
            let deployment = server(database);
            let alice = user(&deployment, "alice@example.com").await;
            let written = create(&deployment, &alice, "before", &[])
                .await
                .expect("a post");

            let updated = update(&deployment, &alice, written.id, "after", &[])
                .await
                .expect("updated");
            assert_eq!(updated.body, "after");
            assert_eq!(updated.inserted_at, written.inserted_at);
            assert!(updated.updated_at >= written.updated_at);
            assert_eq!(updated.version, 2);
            assert_eq!(
                get(&deployment, &alice, written.id)
                    .await
                    .expect("the post")
                    .post,
                updated
            );

            let same = update(&deployment, &alice, written.id, "after", &[])
                .await
                .expect("unchanged");
            assert_eq!(same.version, 2);

            let error = update(&deployment, &alice, written.id, "", &[])
                .await
                .expect_err("blank");
            assert!(matches!(
                error,
                PostError::Validation(PostInvalid::BlankBody)
            ));
            let error = update(&deployment, &alice, Uuid::now_v7(), "x", &[])
                .await
                .expect_err("missing");
            assert!(matches!(error, PostError::NotFound { .. }));
        })
        .await;
    }

    #[tokio::test]
    async fn posts_delete_hides_the_post_and_records_who() {
        with_each_database(|database| async move {
            let deployment = server(database);
            let alice = user(&deployment, "alice@example.com").await;
            let written = create(&deployment, &alice, "going", &[])
                .await
                .expect("a post");

            delete(&deployment, &alice, written.id)
                .await
                .expect("deleted");
            let error = get(&deployment, &alice, written.id)
                .await
                .expect_err("gone");
            assert!(matches!(error, PostError::NotFound { .. }));
            assert!(
                list(&deployment, &alice)
                    .await
                    .expect("the posts")
                    .is_empty()
            );
            let error = delete(&deployment, &alice, written.id)
                .await
                .expect_err("already gone");
            assert!(matches!(error, PostError::NotFound { .. }));

            let mut tx = deployment.begin().await.expect("a transaction");
            let row = get_in(&mut tx, written.id)
                .await
                .expect("the row")
                .expect("kept");
            assert!(row.deleted_at.is_some());
            assert_eq!(row.deleted_by_id, Some(alice.id));
            assert_eq!(row.version, 2);
        })
        .await;
    }

    #[tokio::test]
    async fn posts_apply_event_answers_a_retry_the_same_without_applying_twice() {
        with_each_database(|database| async move {
            let deployment = server(database);
            let alice = user(&deployment, "alice@example.com").await;
            let post = create(&deployment, &alice, "before", &[])
                .await
                .expect("a post");
            let event = Event::update(
                TABLE,
                post.id,
                1,
                json::object_fields(json!({ "body": "before" })),
                json::object_fields(json!({ "body": "after" })),
                now(),
            );

            let first = apply_event(&deployment, &alice, &event)
                .await
                .expect("applied");
            let again = apply_event(&deployment, &alice, &event)
                .await
                .expect("answered again");
            assert_eq!(first.status, AnswerStatus::Applied);
            assert_eq!(again.version, first.version);
            assert_eq!(
                get(&deployment, &alice, post.id)
                    .await
                    .expect("the post")
                    .post
                    .version,
                2
            );
        })
        .await;
    }

    #[tokio::test]
    async fn posts_apply_event_refuses_a_field_it_may_not_change() {
        with_each_database(|database| async move {
            let deployment = server(database);
            let alice = user(&deployment, "alice@example.com").await;
            let post = create(&deployment, &alice, "a post", &[])
                .await
                .expect("a post");
            let event = Event::update(
                TABLE,
                post.id,
                1,
                json::object_fields(json!({})),
                json::object_fields(json!({ "user_id": Uuid::now_v7(), "body": "x" })),
                now(),
            );

            let answer = apply_event(&deployment, &alice, &event)
                .await
                .expect("answered");
            assert_eq!(answer.status, AnswerStatus::Rejected);
            assert_eq!(answer.reason.as_deref(), Some("not_editable: user_id"));
            assert_eq!(
                get(&deployment, &alice, post.id)
                    .await
                    .expect("the post")
                    .post
                    .body,
                "a post"
            );
        })
        .await;
    }

    #[tokio::test]
    async fn posts_apply_event_refuses_another_users_change_with_the_servers_row() {
        with_each_database(|database| async move {
            let deployment = server(database);
            let alice = user(&deployment, "alice@example.com").await;
            let bob = user(&deployment, "bob@example.com").await;
            let post = create(&deployment, &alice, "alice's", &[])
                .await
                .expect("a post");
            let event = Event::update(
                TABLE,
                post.id,
                1,
                json::object_fields(json!({ "body": "alice's" })),
                json::object_fields(json!({ "body": "bob's" })),
                now(),
            );

            let answer = apply_event(&deployment, &bob, &event)
                .await
                .expect("answered");
            assert_eq!(answer.status, AnswerStatus::Rejected);
            assert_eq!(answer.reason.as_deref(), Some("not_owner"));
            assert_eq!(
                answer.server.expect("the row").row["body"],
                json!("alice's")
            );
            let error = update(&deployment, &bob, post.id, "bob's", &[])
                .await
                .expect_err("not bob's");
            assert!(matches!(error, PostError::NotFound { .. }));
        })
        .await;
    }

    #[tokio::test]
    async fn posts_apply_event_refuses_an_invalid_create() {
        with_each_database(|database| async move {
            let deployment = server(database);
            let alice = user(&deployment, "alice@example.com").await;
            let event = Event::create(
                TABLE,
                Uuid::now_v7(),
                json::object_fields(json!({ "body": "  " })),
                now(),
            );

            let answer = apply_event(&deployment, &alice, &event)
                .await
                .expect("answered");
            assert_eq!(answer.status, AnswerStatus::Rejected);
            assert_eq!(
                answer.reason.as_deref(),
                Some("invalid: a post needs some text")
            );
            assert!(
                list(&deployment, &alice)
                    .await
                    .expect("the posts")
                    .is_empty()
            );
        })
        .await;
    }
}
