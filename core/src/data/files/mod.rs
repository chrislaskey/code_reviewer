//! Transaction-scoped file records. Workflows own validation and commits.

use crate::database::Transaction;
use crate::files::{AcceptedFile, File};
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

/// Lists the live files the user sees, newest first with an ID tie-breaker.
pub(crate) async fn list(tx: &mut Transaction, user_id: Uuid) -> Result<Vec<File>, sqlx::Error> {
    query!(tx, list_visible(user_id))
}

/// Lists the live files the user sees with these bytes, newest first.
pub(crate) async fn list_with_sha256(
    tx: &mut Transaction,
    user_id: Uuid,
    sha256: &str,
) -> Result<Vec<File>, sqlx::Error> {
    query!(tx, list_visible_with_sha256(user_id, sha256))
}

/// Gets a live file the user sees.
pub(crate) async fn get(
    tx: &mut Transaction,
    user_id: Uuid,
    id: Uuid,
) -> Result<Option<File>, sqlx::Error> {
    query!(tx, get_visible(user_id, id))
}

/// Persists a row accepted by the resource workflow.
pub(crate) async fn create(
    tx: &mut Transaction,
    file: AcceptedFile<'_>,
) -> Result<(), sqlx::Error> {
    query!(tx, insert(file.row()))
}

/// Persists a row accepted by the resource workflow.
pub(crate) async fn update(
    tx: &mut Transaction,
    file: AcceptedFile<'_>,
) -> Result<bool, sqlx::Error> {
    query!(tx, update(file.row()))
}

/// Explicit replication access for internal resource workflows.
pub(crate) mod replication {
    use super::*;

    /// Reads protocol state, including tombstones, in the caller transaction.
    pub(crate) async fn get(tx: &mut Transaction, id: Uuid) -> Result<Option<File>, sqlx::Error> {
        query!(tx, get(id))
    }

    /// Reads protocol state, including tombstones, in the caller transaction.
    pub(crate) async fn list_synced_after(
        tx: &mut Transaction,
        user_id: Uuid,
        after: &Cursor,
        limit: usize,
    ) -> Result<Vec<File>, sqlx::Error> {
        query!(tx, list_synced_after(user_id, after, limit))
    }

    /// Persists a row accepted by the resource workflow.
    pub(crate) async fn upsert(
        tx: &mut Transaction,
        file: AcceptedFile<'_>,
    ) -> Result<(), sqlx::Error> {
        query!(tx, upsert(file.row()))
    }
}

/// Explicit maintenance access for internal resource workflows.
pub(crate) mod maintenance {
    use super::*;

    /// Lists row IDs, creators and deletion state for byte cleanup.
    pub(crate) async fn list_ids(
        tx: &mut Transaction,
    ) -> Result<Vec<(Uuid, Uuid, bool)>, sqlx::Error> {
        query!(tx, list_ids())
    }

    /// Lists the files on this database the user does not see, for a prune.
    pub(crate) async fn list_invisible_ids(
        tx: &mut Transaction,
        user_id: Uuid,
    ) -> Result<Vec<Uuid>, sqlx::Error> {
        query!(tx, list_invisible_ids(user_id))
    }

    /// Physically removes a refused, forgotten or pruned local file.
    pub(crate) async fn delete(tx: &mut Transaction, id: Uuid) -> Result<(), sqlx::Error> {
        query!(tx, delete_row(id))
    }
}
