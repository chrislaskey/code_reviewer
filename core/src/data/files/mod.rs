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

/// Lists an owner's live files, newest first with an ID tie-breaker.
pub(crate) async fn list(tx: &mut Transaction, user_id: Uuid) -> Result<Vec<File>, sqlx::Error> {
    query!(tx, list_owned(user_id))
}

/// Gets a live file owned by the acting user.
pub(crate) async fn get(
    tx: &mut Transaction,
    user_id: Uuid,
    id: Uuid,
) -> Result<Option<File>, sqlx::Error> {
    Ok(query!(tx, get(id))?.filter(|file| file.deleted_at.is_none() && file.user_id == user_id))
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

    /// Lists row IDs, owners and deletion state for byte cleanup.
    pub(crate) async fn list_ids(
        tx: &mut Transaction,
    ) -> Result<Vec<(Uuid, Uuid, bool)>, sqlx::Error> {
        query!(tx, list_ids())
    }

    /// Physically removes a refused or forgotten local file.
    pub(crate) async fn delete(tx: &mut Transaction, id: Uuid) -> Result<(), sqlx::Error> {
        query!(tx, delete_row(id))
    }
}
