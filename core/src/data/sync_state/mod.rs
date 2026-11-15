//! Transaction-scoped sync state storage. Workflows own policy and commits.

use crate::database::Transaction;
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

/// Gets the stored pull cursor, or none before the first pull.
pub(crate) async fn get(tx: &mut Transaction) -> Result<Option<Cursor>, sqlx::Error> {
    query!(tx, cursor())
}

/// Upserts the pull cursor in the same transaction as its applied page.
pub(crate) async fn upsert(tx: &mut Transaction, cursor: &Cursor) -> Result<(), sqlx::Error> {
    query!(tx, save_cursor(cursor))
}

/// Explicit maintenance operations; bulk SQL and cascades do not run per-row hooks.
pub(crate) mod maintenance {
    use super::*;

    /// Resets device sync rows while preserving the existing pending/conflicted-row rules.
    pub(crate) async fn reset(tx: &mut Transaction, user_id: Uuid) -> Result<(), sqlx::Error> {
        query!(tx, reset(user_id))
    }
}
