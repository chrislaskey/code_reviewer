//! Transaction-scoped local users storage. Workflows own policy and commits.

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

/// Counts the local user's live rows in the tables, each named with
/// the column naming its maker, before conversion.
pub(crate) async fn count_items(
    tx: &mut Transaction,
    owner_columns: &[(&str, &str)],
    user_id: Uuid,
) -> Result<i64, sqlx::Error> {
    query!(tx, count_items(owner_columns, user_id))
}

/// Explicit maintenance operations; bulk SQL and cascades do not run per-row hooks.
pub(crate) mod maintenance {
    use super::*;

    /// Atomically transfers local ownership: each table's column naming
    /// its maker, and the events.
    pub(crate) async fn give_rows_to_user(
        tx: &mut Transaction,
        owner_columns: &[(&str, &str)],
        from: Uuid,
        to: Uuid,
    ) -> Result<(), sqlx::Error> {
        query!(tx, give_rows_to_user(owner_columns, from, to))
    }

    /// Removes local tombstones during account conversion, from the
    /// tables in the order given, children first.
    pub(crate) async fn delete_deleted_rows(
        tx: &mut Transaction,
        tables: &[&str],
    ) -> Result<(), sqlx::Error> {
        query!(tx, delete_deleted_rows(tables))
    }

    /// Removes a converted local user and its remaining cascading rows.
    pub(crate) async fn delete_user(tx: &mut Transaction, id: Uuid) -> Result<(), sqlx::Error> {
        query!(tx, delete_user(id))
    }
}
