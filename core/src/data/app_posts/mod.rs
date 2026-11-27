//! Post records: the `Post` schema, its baseline validation, and the
//! reads and writes of `app_posts`. Ordinary reads exclude tombstones;
//! protocol and physical deletion operations are explicit. Callers own
//! and commit the transaction. Writes are crate-private: a post syncs, so
//! each write is an event applied by `app::posts`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::data::app_sync_changes;
use crate::database::Transaction;
use crate::deployment::Role;

pub const TABLE: &str = "app_posts";
/// The longest body, in characters.
pub const MAX_BODY_LENGTH: usize = 10_000;

/// A post as the pages show it, with the sync columns every synced row
/// carries, see `docs/design/offline-sync.md`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Post {
    pub id: Uuid,
    pub user_id: Uuid,
    pub body: String,
    pub inserted_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// The server's version, 0 on a device's post not yet pushed.
    pub version: i64,
    /// When the server last wrote it; none on a device's post not yet pushed.
    pub last_synced_at: Option<DateTime<Utc>>,
    #[serde(with = "crate::sync::version::optional")]
    pub global_sync_version: Option<i64>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub deleted_by_id: Option<Uuid>,
}

/// Why a post may not be written.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PostInvalid {
    #[error("a post needs some text")]
    BlankBody,
    #[error("a post has {MAX_BODY_LENGTH} characters at most")]
    BodyTooLong,
}

/// A post that passed validation, which `create` and `update` require.
pub struct AcceptedPost(Post);

impl AcceptedPost {
    /// Checks the post, with its body as it is kept.
    pub fn new(post: Post) -> Result<AcceptedPost, PostInvalid> {
        let body = checked_body(&post.body)?;
        Ok(AcceptedPost(Post { body, ..post }))
    }

    /// The accepted post.
    pub fn row(&self) -> &Post {
        &self.0
    }

    /// The accepted post, owned.
    pub fn into_row(self) -> Post {
        self.0
    }
}

/// The body as it is kept: trimmed, not blank and not too long.
pub fn checked_body(body: &str) -> Result<String, PostInvalid> {
    let body = body.trim();
    if body.is_empty() {
        return Err(PostInvalid::BlankBody);
    }
    if body.chars().count() > MAX_BODY_LENGTH {
        return Err(PostInvalid::BodyTooLong);
    }
    Ok(body.to_string())
}

#[cfg(feature = "postgres")]
mod postgres;
mod sqlite;

macro_rules! query {
    ($tx:expr, $function:ident($($argument:expr),* $(,)?)) => {
        match $tx {
            Transaction::Sqlite(tx) => sqlite::$function(&mut **tx, $($argument),*).await,
            #[cfg(feature = "postgres")]
            Transaction::Postgres(tx) => postgres::$function(&mut **tx, $($argument),*).await,
        }
    };
}

/// A required read scope, with the existing ordering for each query.
pub enum Filter {
    /// Visible posts, newest first, with an ID tie-breaker.
    VisibleTo(Uuid),
    /// Owned posts, oldest first, with an ID tie-breaker.
    OwnedBy(Uuid),
}

/// Lists live rows within the required scope.
pub async fn list(tx: &mut Transaction, filter: Filter) -> Result<Vec<Post>, sqlx::Error> {
    match filter {
        Filter::VisibleTo(user_id) => query!(tx, list_visible(user_id)),
        Filter::OwnedBy(user_id) => query!(tx, list_live_of_user(user_id)),
    }
}

/// Gets a visible live row; inaccessible and deleted rows are absent.
pub async fn get(
    tx: &mut Transaction,
    user_id: Uuid,
    id: Uuid,
) -> Result<Option<Post>, sqlx::Error> {
    Ok(query!(tx, get_visible(user_id, id))?.filter(|post| post.deleted_at.is_none()))
}

/// Inserts a new row. On the server this publishes it: the row gets the
/// next global sync version, recorded in `app_sync_changes` in this
/// transaction. On a device the row keeps the version it has.
pub(crate) async fn create(
    tx: &mut Transaction,
    role: Role,
    post: AcceptedPost,
) -> Result<Post, sqlx::Error> {
    let mut post = post.into_row();
    post.global_sync_version =
        app_sync_changes::stamp(tx, role, TABLE, post.id, post.global_sync_version).await?;
    query!(tx, insert(&post))?;
    Ok(post)
}

/// Writes the row over its stored one, a soft delete included, and
/// publishes it on the server as `create` does.
pub(crate) async fn update(
    tx: &mut Transaction,
    role: Role,
    post: AcceptedPost,
) -> Result<Post, sqlx::Error> {
    let mut post = post.into_row();
    post.global_sync_version =
        app_sync_changes::stamp(tx, role, TABLE, post.id, post.global_sync_version).await?;
    query!(tx, update(&post))?;
    Ok(post)
}

/// Protocol reads include tombstones; authoritative writes retain server versions.
pub mod replication {
    use super::*;
    use crate::sync::VersionRange;

    /// Gets a row regardless of visibility or deletion for event processing.
    pub async fn get(tx: &mut Transaction, id: Uuid) -> Result<Option<Post>, sqlx::Error> {
        query!(tx, get(id))
    }

    /// Lists a visible page including tombstones in cursor order.
    pub async fn list(
        tx: &mut Transaction,
        user_id: Uuid,
        range: &VersionRange,
        limit: usize,
    ) -> Result<Vec<Post>, sqlx::Error> {
        query!(tx, list_synced_after(user_id, range, limit))
    }

    /// Persists the server's row as it is, the server being the system of
    /// record, after the caller checks pending changes.
    pub(crate) async fn upsert(tx: &mut Transaction, post: &Post) -> Result<(), sqlx::Error> {
        query!(tx, upsert(post))
    }
}

/// Physical device cleanup; these operations never represent user deletion.
pub(crate) mod maintenance {
    use super::*;

    /// Removes a refused local create and its cascading association rows.
    pub(crate) async fn delete(tx: &mut Transaction, id: Uuid) -> Result<(), sqlx::Error> {
        query!(tx, delete_row(id))
    }

    /// Removes unreachable device rows and their cascading associations.
    pub(crate) async fn prune(tx: &mut Transaction, user_id: Uuid) -> Result<u64, sqlx::Error> {
        query!(tx, delete_unreachable(user_id))
    }
}
