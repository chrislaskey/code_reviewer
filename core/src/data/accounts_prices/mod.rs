//! Price records: the `Price` schema and the reads and writes of
//! `accounts_prices`. Callers own and commit the transaction.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::database::Transaction;

pub const TABLE: &str = "accounts_prices";

/// How often a plan's price is paid; a one-time price has none.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PriceInterval {
    Month,
    Year,
}

impl PriceInterval {
    /// Its name in `accounts_prices`.
    pub fn as_str(self) -> &'static str {
        match self {
            PriceInterval::Month => "month",
            PriceInterval::Year => "year",
        }
    }

    /// Stripe's word, or the column's; none for any other.
    pub fn from_name(name: &str) -> Option<PriceInterval> {
        match name {
            "month" => Some(PriceInterval::Month),
            "year" => Some(PriceInterval::Year),
            _ => None,
        }
    }
}

/// A price of a product, synced from Stripe.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Price {
    pub id: Uuid,
    pub product_id: Uuid,
    /// `price_...`
    pub stripe_id: String,
    /// What the code knows the price by, see `products::Grant`.
    pub lookup_key: Option<String>,
    /// In the currency's smallest unit, cents for `usd`.
    pub amount: i64,
    /// Lowercase, `usd`.
    pub currency: String,
    pub interval: Option<PriceInterval>,
    pub active: bool,
    pub inserted_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
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
pub async fn get(tx: &mut Transaction, id: Uuid) -> Result<Option<Price>, sqlx::Error> {
    query!(tx, get(id))
}

/// Gets a stored provider record by its Stripe ID.
pub async fn get_by_stripe_id(
    tx: &mut Transaction,
    stripe_id: &str,
) -> Result<Option<Price>, sqlx::Error> {
    query!(tx, get_by_stripe_id(stripe_id))
}

/// Lists stored catalog records in their existing order.
pub async fn list(tx: &mut Transaction) -> Result<Vec<Price>, sqlx::Error> {
    query!(tx, list())
}

/// Persists a record accepted by the resource workflow.
pub async fn upsert(tx: &mut Transaction, price: &Price) -> Result<(), sqlx::Error> {
    query!(tx, upsert(price))
}

/// Explicit maintenance access for trusted payment workflows.
pub mod maintenance {
    use super::*;

    /// Deactivates the previous catalog snapshot within the refresh transaction.
    pub async fn deactivate_all(
        tx: &mut Transaction,
        now: DateTime<Utc>,
    ) -> Result<(), sqlx::Error> {
        query!(tx, deactivate_all(now))
    }
}
