//! Transaction-scoped device settings storage. Workflows own policy and commits.

use crate::database::Transaction;
use serde_json::Value;

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

/// Gets one stored record in the caller transaction.
pub(crate) async fn get(tx: &mut Transaction, key: &str) -> Result<Option<Value>, sqlx::Error> {
    query!(tx, get(key))
}

/// Upserts a JSON setting; serialization and defaults belong to its workflow.
pub(crate) async fn upsert(
    tx: &mut Transaction,
    key: &str,
    value: &Value,
) -> Result<(), sqlx::Error> {
    query!(tx, set(key, value))
}
