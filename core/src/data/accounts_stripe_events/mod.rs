//! Stripe event receipts: the `StripeEvent` schema and the reads and
//! writes of `accounts_stripe_events`. Callers own and commit the
//! transaction.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::database::Transaction;

pub const TABLE: &str = "accounts_stripe_events";

/// An event handled, by Stripe's id, `evt_...`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StripeEvent {
    pub id: String,
    /// Stripe's name for it, `checkout.session.completed`.
    pub event_type: String,
    pub handled_at: DateTime<Utc>,
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

/// Gets one stored record by ID.
pub async fn get(tx: &mut Transaction, id: &str) -> Result<Option<StripeEvent>, sqlx::Error> {
    query!(tx, get(id))
}

/// Records a store payment or delivery only when its unique key is new.
pub async fn create(tx: &mut Transaction, receipt: &StripeEvent) -> Result<u64, sqlx::Error> {
    query!(
        tx,
        insert_if_missing(&receipt.id, &receipt.event_type, receipt.handled_at)
    )
}
