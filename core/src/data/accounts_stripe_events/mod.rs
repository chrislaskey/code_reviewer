//! Transaction-scoped stripe events records. Workflows own policy and commits.

use crate::accounts::stripe_events::{AcceptedStripeEvent, StripeEvent};
use crate::database::Transaction;

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

/// Gets one stored record by ID.
pub(crate) async fn get(
    tx: &mut Transaction,
    id: &str,
) -> Result<Option<StripeEvent>, sqlx::Error> {
    query!(tx, get(id))
}

/// Records a store payment or delivery only when its unique key is new.
pub(crate) async fn create(
    tx: &mut Transaction,
    receipt: AcceptedStripeEvent<'_>,
) -> Result<u64, sqlx::Error> {
    query!(
        tx,
        insert_if_missing(
            &receipt.row().id,
            &receipt.row().event_type,
            receipt.row().handled_at
        )
    )
}
