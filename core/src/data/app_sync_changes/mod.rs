//! The server's change log: one row of `app_sync_changes` per published
//! change, by its global sync version, naming the table and row it
//! changed. It holds no row data. Callers own and commit the transaction.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::data::app_sync_versions;
use crate::database::Transaction;
use crate::deployment::{Role, now};

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

/// Publishes a change to a row on the server: gives out the next global
/// sync version and records the change under it. None on a device,
/// which publishes nothing.
pub(crate) async fn record(
    tx: &mut Transaction,
    role: Role,
    table: &str,
    row_id: Uuid,
) -> Result<Option<i64>, sqlx::Error> {
    if role == Role::Device {
        return Ok(None);
    }
    let version = app_sync_versions::next(tx).await?;
    query!(tx, insert(version, table, row_id, now()))?;
    Ok(Some(version))
}

/// The global sync version to store on a row being written: a new one,
/// recorded with its change, on the server; the row's own on a device.
pub(crate) async fn stamp(
    tx: &mut Transaction,
    role: Role,
    table: &str,
    row_id: Uuid,
    current: Option<i64>,
) -> Result<Option<i64>, sqlx::Error> {
    Ok(record(tx, role, table, row_id).await?.or(current))
}

/// Explicit maintenance operations.
pub(crate) mod maintenance {
    use super::*;

    /// Deletes the oldest changes, up to the first one written at or
    /// after `horizon`, and moves the feed's floor to the last one
    /// deleted. Returns the floor.
    pub(crate) async fn delete_written_before(
        tx: &mut Transaction,
        horizon: DateTime<Utc>,
    ) -> Result<i64, sqlx::Error> {
        let feed = app_sync_versions::get(tx).await?;
        let first_kept = query!(tx, first_written_at_or_after(horizon))?;
        let floor = first_kept.map_or(feed.head, |version| version - 1);
        query!(tx, delete_through(floor))?;
        app_sync_versions::update_floor(tx, floor).await?;
        Ok(floor)
    }
}

/// The rows of `table` on the server whose global sync version is not
/// the latest change recorded for them: a write that skipped `record`.
#[cfg(test)]
pub(crate) async fn list_unpublished(
    tx: &mut Transaction,
    table: &str,
) -> Result<Vec<String>, sqlx::Error> {
    query!(tx, list_unpublished(table))
}
