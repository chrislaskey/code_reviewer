//! Transaction-scoped credential records. Workflows own commits.

use crate::credentials::AcceptedPasswordHash;
use crate::database::Transaction;
use chrono::{DateTime, Utc};
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

/// Stores only a password hash produced by the credential workflow.
pub(crate) async fn upsert(
    tx: &mut Transaction,
    user_id: Uuid,
    password_hash: AcceptedPasswordHash<'_>,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    query!(tx, upsert(user_id, password_hash.value(), now))
}

/// Gets the stored encoded hash for password verification only.
pub(crate) async fn get(
    tx: &mut Transaction,
    user_id: Uuid,
) -> Result<Option<String>, sqlx::Error> {
    query!(tx, get_password_hash(user_id))
}
