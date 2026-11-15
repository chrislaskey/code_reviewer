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

/// Counts the local user's live posts and files before conversion.
pub(crate) async fn count_items(tx: &mut Transaction, user_id: Uuid) -> Result<i64, sqlx::Error> {
    query!(tx, count_items(user_id))
}

/// Explicit maintenance operations; bulk SQL and cascades do not run per-row hooks.
pub(crate) mod maintenance {
    use super::*;

    /// Atomically transfers local ownership using the existing multi-table statements.
    pub(crate) async fn give_rows_to_user(
        tx: &mut Transaction,
        from: Uuid,
        to: Uuid,
    ) -> Result<(), sqlx::Error> {
        query!(tx, give_rows_to_user(from, to))
    }

    /// Removes local tombstones during account conversion.
    pub(crate) async fn delete_deleted_rows(tx: &mut Transaction) -> Result<(), sqlx::Error> {
        query!(tx, delete_deleted_rows())
    }

    /// Removes a converted local user and its remaining cascading rows.
    pub(crate) async fn delete_user(tx: &mut Transaction, id: Uuid) -> Result<(), sqlx::Error> {
        query!(tx, delete_user(id))
    }
}
