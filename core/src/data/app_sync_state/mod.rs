//! The device's place in the server's rows: the `Cursor` it pulled to,
//! one row of `app_sync_state` per user, and the reads and writes of it.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::database::Transaction;
use crate::sync::Continuation;

/// The part of a server feed already applied to the device.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cursor {
    pub feed_id: Uuid,
    #[serde(with = "crate::sync::version")]
    pub global_sync_version: i64,
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

/// Gets the stored pull cursor, or none before the first pull.
pub async fn get(tx: &mut Transaction) -> Result<Option<Cursor>, sqlx::Error> {
    query!(tx, cursor())
}

/// Upserts the pull cursor in the same transaction as its applied page.
pub async fn upsert(tx: &mut Transaction, cursor: &Cursor) -> Result<(), sqlx::Error> {
    query!(tx, save_cursor(cursor))
}

/// Gets the continuation of an unfinished first pull, which the next
/// sync asks for instead of starting over.
pub(crate) async fn get_continuation(
    tx: &mut Transaction,
) -> Result<Option<Continuation>, sqlx::Error> {
    query!(tx, continuation())?
        .map(|json| from_json(&json))
        .transpose()
}

/// Upserts the continuation of an unfinished first pull, in the same
/// transaction as its applied page. Saving the cursor clears it.
pub(crate) async fn upsert_continuation(
    tx: &mut Transaction,
    continuation: &Continuation,
) -> Result<(), sqlx::Error> {
    query!(tx, save_continuation(&to_json(continuation)?))
}

/// A continuation as the device keeps it.
pub(crate) fn to_json(continuation: &Continuation) -> Result<String, sqlx::Error> {
    serde_json::to_string(continuation).map_err(|error| sqlx::Error::Encode(Box::new(error)))
}

/// A continuation the device kept.
pub(crate) fn from_json(json: &str) -> Result<Continuation, sqlx::Error> {
    serde_json::from_str(json).map_err(|error| sqlx::Error::Decode(Box::new(error)))
}

/// Explicit maintenance operations; bulk SQL and cascades do not run per-row hooks.
pub mod maintenance {
    use super::*;

    /// Drops a synced table's rows for a resync, but those a pending
    /// event is about and those whose create was refused.
    pub async fn delete_not_pending_or_refused(
        tx: &mut Transaction,
        table: &str,
    ) -> Result<(), sqlx::Error> {
        query!(tx, delete_not_pending_or_refused(table))
    }

    /// Drops the cursor and the backfills to do, for a resync.
    pub async fn delete_cursor_and_backfills(tx: &mut Transaction) -> Result<(), sqlx::Error> {
        query!(tx, delete_cursor_and_backfills())
    }

    /// Clears old feed stamps from rows retained during a full resync.
    pub(crate) async fn clear_versions(
        tx: &mut Transaction,
        table: &str,
    ) -> Result<(), sqlx::Error> {
        query!(tx, clear_versions(table))
    }
}

/// Reads a device row's last version before applying a server snapshot.
pub(crate) async fn row_version(
    tx: &mut Transaction,
    table: &str,
    id: Uuid,
) -> Result<Option<i64>, sqlx::Error> {
    query!(tx, row_version(table, id))
}
