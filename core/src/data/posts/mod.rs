//! Post records. Ordinary reads exclude tombstones; protocol and physical
//! deletion operations are explicit. Callers own and commit the transaction.

use uuid::Uuid;

use crate::app::posts::{AcceptedPost, Post};
use crate::database::Transaction;

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
pub(crate) enum Filter {
    /// Visible posts, newest first, with an ID tie-breaker.
    VisibleTo(Uuid),
    /// Owned posts, oldest first, with an ID tie-breaker.
    OwnedBy(Uuid),
}

/// Lists live rows within the required scope.
pub(crate) async fn list(tx: &mut Transaction, filter: Filter) -> Result<Vec<Post>, sqlx::Error> {
    match filter {
        Filter::VisibleTo(user_id) => query!(tx, list_visible(user_id)),
        Filter::OwnedBy(user_id) => query!(tx, list_live_of_user(user_id)),
    }
}

/// Gets a visible live row; inaccessible and deleted rows are absent.
pub(crate) async fn get(
    tx: &mut Transaction,
    user_id: Uuid,
    id: Uuid,
) -> Result<Option<Post>, sqlx::Error> {
    Ok(query!(tx, get_visible(user_id, id))?.filter(|post| post.deleted_at.is_none()))
}

/// Persists a create accepted by the event handler.
pub(crate) async fn create(
    tx: &mut Transaction,
    post: AcceptedPost<'_>,
) -> Result<(), sqlx::Error> {
    query!(tx, insert(post.row()))
}

/// Persists an accepted update, including a soft delete.
pub(crate) async fn update(
    tx: &mut Transaction,
    post: AcceptedPost<'_>,
) -> Result<bool, sqlx::Error> {
    query!(tx, update(post.row()))
}

/// Protocol reads include tombstones; authoritative writes retain server versions.
pub(crate) mod replication {
    use super::*;
    use crate::app::sync::Cursor;

    /// Gets a row regardless of visibility or deletion for event processing.
    pub(crate) async fn get(tx: &mut Transaction, id: Uuid) -> Result<Option<Post>, sqlx::Error> {
        query!(tx, get(id))
    }

    /// Lists a visible page including tombstones in cursor order.
    pub(crate) async fn list(
        tx: &mut Transaction,
        user_id: Uuid,
        after: &Cursor,
        limit: usize,
    ) -> Result<Vec<Post>, sqlx::Error> {
        query!(tx, list_synced_after(user_id, after, limit))
    }

    /// Persists an authoritative row after the caller checks pending changes.
    pub(crate) async fn upsert(
        tx: &mut Transaction,
        post: AcceptedPost<'_>,
    ) -> Result<(), sqlx::Error> {
        query!(tx, upsert(post.row()))
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
