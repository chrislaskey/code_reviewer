//! Transaction-scoped file-community association records. Workflows own commits.

use crate::database::Transaction;
use crate::files_communities::{AcceptedFileCommunity, FileCommunity};
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

/// A required relationship scope for internal association reads.
pub(crate) enum Filter<'a> {
    /// Every live row of the files, in every community.
    Files(&'a [Uuid]),
    /// The live rows of the files in the communities the user is in.
    FilesVisibleTo {
        user_id: Uuid,
        file_ids: &'a [Uuid],
    },
    Community(Uuid),
}

/// Lists live association rows within the workflow's relationship scope.
pub(crate) async fn list(
    tx: &mut Transaction,
    filter: Filter<'_>,
) -> Result<Vec<FileCommunity>, sqlx::Error> {
    match filter {
        Filter::Files([]) | Filter::FilesVisibleTo { file_ids: [], .. } => Ok(Vec::new()),
        Filter::Files(ids) => query!(tx, list_live_of_files(ids)),
        Filter::FilesVisibleTo { user_id, file_ids } => {
            query!(tx, list_live_visible_of_files(user_id, file_ids))
        }
        Filter::Community(id) => query!(tx, list_live_of_community(id)),
    }
}

/// Persists a row accepted by the resource workflow.
pub(crate) async fn create(
    tx: &mut Transaction,
    row: AcceptedFileCommunity<'_>,
) -> Result<(), sqlx::Error> {
    query!(tx, insert(row.row()))
}

/// Persists a row accepted by the resource workflow.
pub(crate) async fn update(
    tx: &mut Transaction,
    row: AcceptedFileCommunity<'_>,
) -> Result<bool, sqlx::Error> {
    query!(tx, update(row.row()))
}

/// Explicit replication access for internal resource workflows.
pub(crate) mod replication {
    use super::*;

    /// Reads protocol state, including tombstones, in the caller transaction.
    pub(crate) async fn get(
        tx: &mut Transaction,
        id: Uuid,
    ) -> Result<Option<FileCommunity>, sqlx::Error> {
        query!(tx, get(id))
    }

    /// Reads protocol state, including tombstones, in the caller transaction.
    pub(crate) async fn get_by_file_and_community(
        tx: &mut Transaction,
        file_id: Uuid,
        community_id: Uuid,
    ) -> Result<Option<FileCommunity>, sqlx::Error> {
        query!(tx, get_by_file_and_community(file_id, community_id))
    }

    /// Reads protocol state, including tombstones, in the caller transaction.
    pub(crate) async fn list_synced_after(
        tx: &mut Transaction,
        user_id: Uuid,
        after: &Cursor,
        limit: usize,
    ) -> Result<Vec<FileCommunity>, sqlx::Error> {
        query!(tx, list_synced_after(user_id, after, limit))
    }

    /// Persists a row accepted by the resource workflow.
    pub(crate) async fn upsert(
        tx: &mut Transaction,
        row: AcceptedFileCommunity<'_>,
    ) -> Result<(), sqlx::Error> {
        query!(tx, upsert(row.row()))
    }

    /// Lists live association rows for a membership-authorized backfill.
    pub(crate) async fn list_live_of_community_after(
        tx: &mut Transaction,
        community_id: Uuid,
        after: &Cursor,
        limit: usize,
    ) -> Result<Vec<FileCommunity>, sqlx::Error> {
        query!(tx, list_live_of_community_after(community_id, after, limit))
    }
}

/// Explicit maintenance access for internal resource workflows.
pub(crate) mod maintenance {
    use super::*;

    /// Physically removes a device's row the server holds under another id.
    pub(crate) async fn delete(tx: &mut Transaction, id: Uuid) -> Result<(), sqlx::Error> {
        query!(tx, delete_row(id))
    }
}
