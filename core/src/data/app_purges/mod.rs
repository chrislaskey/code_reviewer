//! Purge records: the `Purge` schema, one row per run, and the reads and
//! writes of `app_purges`, with the bulk removals a run makes. Callers own
//! and commit the transaction.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::database::Transaction;

/// One run as `app_purges` keeps it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Purge {
    pub id: Uuid,
    pub horizon: DateTime<Utc>,
    pub inserted_at: DateTime<Utc>,
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

/// Persists the record accepted by its workflow.
pub async fn create(tx: &mut Transaction, purge: &Purge) -> Result<(), sqlx::Error> {
    query!(tx, insert(purge))
}

/// Gets the most recent purge horizon, or none before the first purge.
pub async fn get(tx: &mut Transaction) -> Result<Option<DateTime<Utc>>, sqlx::Error> {
    query!(tx, latest_horizon())
}

/// Explicit maintenance operations; bulk SQL and cascades do not run per-row hooks.
pub(crate) mod maintenance {
    use super::*;

    /// Physically removes a synced table's tombstones older than the horizon.
    pub(crate) async fn delete_deleted_before(
        tx: &mut Transaction,
        table: &str,
        horizon: DateTime<Utc>,
    ) -> Result<u64, sqlx::Error> {
        query!(tx, delete_deleted_before(table, horizon))
    }
}
