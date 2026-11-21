//! Staff records: the `Staff` schema and the reads and writes of
//! `ops_staff`. Callers own and commit the transaction.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::database::Transaction;

pub const TABLE: &str = "ops_staff";

/// A staff member: an account that may use the ops app.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Staff {
    pub user_id: Uuid,
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

/// Gets the staff assignment for one account.
pub async fn get(tx: &mut Transaction, user_id: Uuid) -> Result<Option<Staff>, sqlx::Error> {
    query!(tx, get(user_id))
}

/// Creates a validated staff assignment idempotently.
pub async fn create(tx: &mut Transaction, staff: &Staff) -> Result<(), sqlx::Error> {
    query!(tx, insert_if_missing(staff.user_id, staff.inserted_at))
}

/// Removes a staff assignment, returning the number removed.
pub async fn delete(tx: &mut Transaction, user_id: Uuid) -> Result<u64, sqlx::Error> {
    query!(tx, delete(user_id))
}
