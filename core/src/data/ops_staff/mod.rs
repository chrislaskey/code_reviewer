//! Transaction-scoped staff records. Workflows own commits.

use crate::database::Transaction;
use crate::ops::staff::{AcceptedStaff, Staff};
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

/// Gets the staff assignment for one account.
pub(crate) async fn get(tx: &mut Transaction, user_id: Uuid) -> Result<Option<Staff>, sqlx::Error> {
    query!(tx, get(user_id))
}

/// Creates a validated staff assignment idempotently.
pub(crate) async fn create(
    tx: &mut Transaction,
    staff: AcceptedStaff<'_>,
) -> Result<(), sqlx::Error> {
    query!(
        tx,
        insert_if_missing(staff.row().user_id, staff.row().inserted_at)
    )
}

/// Removes a staff assignment, returning the number removed.
pub(crate) async fn delete(tx: &mut Transaction, user_id: Uuid) -> Result<u64, sqlx::Error> {
    query!(tx, delete(user_id))
}
