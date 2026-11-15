//! Transaction-scoped community records. Workflows own commits.

use crate::communities::{AcceptedCommunity, Community};
use crate::database::Transaction;
use crate::sync::Cursor;
use uuid::Uuid;

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

/// Lists live communities visible to this user, ordered by name and ID.
pub(crate) async fn list(
    tx: &mut Transaction,
    user_id: Uuid,
) -> Result<Vec<Community>, sqlx::Error> {
    query!(tx, list_live(user_id))
}

/// Gets a live community visible to this user, including trial isolation.
pub(crate) async fn get(
    tx: &mut Transaction,
    user_id: Uuid,
    id: Uuid,
) -> Result<Option<Community>, sqlx::Error> {
    let row = query!(&mut *tx, get(id))?.filter(|row| row.deleted_at.is_none());
    if row.is_some() && crate::data::trial_users::community_is_hidden_from(tx, id, user_id).await? {
        return Ok(None);
    }
    Ok(row)
}

/// Persists a row accepted by the resource workflow.
pub(crate) async fn create(
    tx: &mut Transaction,
    community: AcceptedCommunity<'_>,
) -> Result<(), sqlx::Error> {
    query!(tx, insert(community.row()))
}

/// Persists a row accepted by the resource workflow.
pub(crate) async fn update(
    tx: &mut Transaction,
    community: AcceptedCommunity<'_>,
) -> Result<(), sqlx::Error> {
    query!(tx, update(community.row()))
}

/// Explicit replication access for internal resource workflows.
pub(crate) mod replication {
    use super::*;

    /// Reads protocol state, including tombstones, in the caller transaction.
    pub(crate) async fn list_by_ids(
        tx: &mut Transaction,
        ids: &[Uuid],
    ) -> Result<Vec<Community>, sqlx::Error> {
        query!(tx, list_by_ids(ids))
    }

    /// Reads protocol state, including tombstones, in the caller transaction.
    pub(crate) async fn get(
        tx: &mut Transaction,
        id: Uuid,
    ) -> Result<Option<Community>, sqlx::Error> {
        query!(tx, get(id))
    }

    /// Reads protocol state, including tombstones, in the caller transaction.
    pub(crate) async fn list_synced_after(
        tx: &mut Transaction,
        user_id: Uuid,
        after: &Cursor,
        limit: usize,
    ) -> Result<Vec<Community>, sqlx::Error> {
        query!(tx, list_synced_after(user_id, after, limit))
    }

    /// Persists a row accepted by the resource workflow.
    pub(crate) async fn upsert(
        tx: &mut Transaction,
        community: AcceptedCommunity<'_>,
    ) -> Result<(), sqlx::Error> {
        query!(tx, upsert(community.row()))
    }
}

/// Explicit maintenance access for internal resource workflows.
pub(crate) mod maintenance {
    use super::*;

    /// Physically removes a refused local row and cascading children.
    pub(crate) async fn delete(tx: &mut Transaction, id: Uuid) -> Result<(), sqlx::Error> {
        query!(tx, delete_row(id))
    }
}
