//! The device's place in the server's rows: the `Cursor` it pulled to,
//! one row of `app_sync_state` per user, and the reads and writes of it.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::database::Transaction;

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

/// Explicit maintenance operations; bulk SQL and cascades do not run per-row hooks.
pub mod maintenance {
    use super::*;

    /// Drops a synced table's rows for a resync, but those a pending
    /// event is about.
    pub async fn delete_not_pending(tx: &mut Transaction, table: &str) -> Result<(), sqlx::Error> {
        query!(tx, delete_not_pending(table))
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
        let table = crate::database::identifier(table)?;
        let statement = format!(
            "UPDATE {table} SET global_sync_version = NULL WHERE global_sync_version IS NOT NULL"
        );
        match tx {
            Transaction::Sqlite(connection) => sqlx::query(sqlx::AssertSqlSafe(statement))
                .execute(&mut **connection)
                .await
                .map(|_| ()),
            #[cfg(feature = "postgres")]
            Transaction::Postgres(connection) => sqlx::query(sqlx::AssertSqlSafe(statement))
                .execute(&mut **connection)
                .await
                .map(|_| ()),
        }
    }
}

/// Reads a device row's last version before applying a server snapshot.
pub(crate) async fn row_version(
    tx: &mut Transaction,
    table: &str,
    id: Uuid,
) -> Result<Option<i64>, sqlx::Error> {
    let table = crate::database::identifier(table)?;
    let statement = format!("SELECT global_sync_version FROM {table} WHERE id = $1");
    let version: Option<Option<i64>> = match tx {
        Transaction::Sqlite(connection) => {
            sqlx::query_scalar(sqlx::AssertSqlSafe(statement))
                .bind(id.hyphenated())
                .fetch_optional(&mut **connection)
                .await?
        }
        #[cfg(feature = "postgres")]
        Transaction::Postgres(connection) => {
            sqlx::query_scalar(sqlx::AssertSqlSafe(statement))
                .bind(id)
                .fetch_optional(&mut **connection)
                .await?
        }
    };
    Ok(version.flatten())
}
