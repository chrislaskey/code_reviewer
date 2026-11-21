//! Transaction-scoped sync backfills storage. Workflows own policy and commits.

use crate::database::Transaction;
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

/// Queues a community backfill idempotently.
pub(crate) async fn create(tx: &mut Transaction, community_id: Uuid) -> Result<(), sqlx::Error> {
    query!(tx, insert_if_missing(community_id))
}

/// Lists queued community backfills in community-ID order.
pub(crate) async fn list(tx: &mut Transaction) -> Result<Vec<Uuid>, sqlx::Error> {
    query!(tx, list())
}

/// Removes the specified stored record.
pub(crate) async fn delete(tx: &mut Transaction, community_id: Uuid) -> Result<(), sqlx::Error> {
    query!(tx, delete(community_id))
}
