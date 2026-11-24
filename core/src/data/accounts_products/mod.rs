//! Product records: the `Product` schema and the reads and writes of
//! `accounts_products`. Callers own and commit the transaction.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::database::Transaction;

pub const TABLE: &str = "accounts_products";

/// A product as Stripe has it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Product {
    pub id: Uuid,
    /// `prod_...`
    pub stripe_id: String,
    pub name: String,
    pub description: Option<String>,
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
pub async fn get(tx: &mut Transaction, id: Uuid) -> Result<Option<Product>, sqlx::Error> {
    query!(tx, get(id))
}

/// Gets a stored provider record by its Stripe ID.
pub async fn get_by_stripe_id(
    tx: &mut Transaction,
    stripe_id: &str,
) -> Result<Option<Product>, sqlx::Error> {
    query!(tx, get_by_stripe_id(stripe_id))
}

/// Lists stored catalog records in their existing order.
pub async fn list(tx: &mut Transaction) -> Result<Vec<Product>, sqlx::Error> {
    query!(tx, list())
}

/// Persists a record accepted by the resource workflow.
pub async fn upsert(tx: &mut Transaction, product: &Product) -> Result<(), sqlx::Error> {
    query!(tx, upsert(product))
}

/// Deactivates one provider product without removing its purchase history.
pub async fn deactivate_by_stripe_id(
    tx: &mut Transaction,
    stripe_id: &str,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    query!(tx, deactivate_by_stripe_id(stripe_id, now))
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
