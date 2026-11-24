//! File community records: the `FileCommunity` schema and the reads and
//! writes of `app_files_communities`. Callers own and commit the
//! transaction. Writes are crate-private: a file community row syncs, so
//! each write is an event applied by `app::files_communities`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::data::app_sync_state::Cursor;
use crate::database::Transaction;

pub const TABLE: &str = "app_files_communities";

/// A file in a community, with the sync columns every synced row carries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileCommunity {
    pub id: Uuid,
    pub file_id: Uuid,
    pub community_id: Uuid,
    /// The member who put the file in the community.
    pub created_by_id: Uuid,
    pub inserted_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub version: i64,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub deleted_by_id: Option<Uuid>,
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

/// A required relationship scope for internal association reads.
pub enum Filter<'a> {
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
pub async fn list(
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
pub(crate) async fn create(tx: &mut Transaction, row: &FileCommunity) -> Result<(), sqlx::Error> {
    query!(tx, insert(row))
}

/// Persists a row accepted by the resource workflow.
pub(crate) async fn update(tx: &mut Transaction, row: &FileCommunity) -> Result<bool, sqlx::Error> {
    query!(tx, update(row))
}

/// Explicit replication access for internal resource workflows.
pub mod replication {
    use super::*;

    /// Reads protocol state, including tombstones, in the caller transaction.
    pub async fn get(tx: &mut Transaction, id: Uuid) -> Result<Option<FileCommunity>, sqlx::Error> {
        query!(tx, get(id))
    }

    /// Reads protocol state, including tombstones, in the caller transaction.
    pub async fn get_by_file_and_community(
        tx: &mut Transaction,
        file_id: Uuid,
        community_id: Uuid,
    ) -> Result<Option<FileCommunity>, sqlx::Error> {
        query!(tx, get_by_file_and_community(file_id, community_id))
    }

    /// Reads protocol state, including tombstones, in the caller transaction.
    pub async fn list_synced_after(
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
        row: &FileCommunity,
    ) -> Result<(), sqlx::Error> {
        query!(tx, upsert(row))
    }

    /// Lists live association rows for a membership-authorized backfill.
    pub async fn list_live_of_community_after(
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

    /// Drops the rows for a resync, but the pending and those of a
    /// pending or refused file.
    pub(crate) async fn delete_for_resync(tx: &mut Transaction) -> Result<(), sqlx::Error> {
        query!(tx, delete_for_resync())
    }

    /// Physically removes a device's row the server holds under another id.
    pub(crate) async fn delete(tx: &mut Transaction, id: Uuid) -> Result<(), sqlx::Error> {
        query!(tx, delete_row(id))
    }

    /// Moves every file from one community to another, when a local
    /// user's communities are merged; a file already in both keeps only
    /// its row in the second.
    pub(crate) async fn move_rows_to_community(
        tx: &mut Transaction,
        from_community_id: Uuid,
        to_community_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        query!(
            tx,
            move_rows_to_community(from_community_id, to_community_id)
        )
    }
}
