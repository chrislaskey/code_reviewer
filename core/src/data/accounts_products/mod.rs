//! Transaction-scoped products records. Workflows own policy and commits.

use crate::accounts::products::{AcceptedProduct, Product};
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

/// Gets one stored record by ID.
pub(crate) async fn get(tx: &mut Transaction, id: Uuid) -> Result<Option<Product>, sqlx::Error> {
    query!(tx, get(id))
}

/// Gets a stored provider record by its Stripe ID.
pub(crate) async fn get_by_stripe_id(
    tx: &mut Transaction,
    stripe_id: &str,
) -> Result<Option<Product>, sqlx::Error> {
    query!(tx, get_by_stripe_id(stripe_id))
}

/// Lists stored catalog records in their existing order.
pub(crate) async fn list(tx: &mut Transaction) -> Result<Vec<Product>, sqlx::Error> {
    query!(tx, list())
}

/// Persists a record accepted by the resource workflow.
pub(crate) async fn upsert(
    tx: &mut Transaction,
    product: AcceptedProduct<'_>,
) -> Result<(), sqlx::Error> {
    query!(tx, upsert(product.row()))
}

/// Deactivates one provider product without removing its purchase history.
pub(crate) async fn deactivate_by_stripe_id(
    tx: &mut Transaction,
    stripe_id: &str,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    query!(tx, deactivate_by_stripe_id(stripe_id, now))
}

/// Explicit maintenance access for trusted payment workflows.
pub(crate) mod maintenance {
    use super::*;

    /// Deactivates the previous catalog snapshot within the refresh transaction.
    pub(crate) async fn deactivate_all(
        tx: &mut Transaction,
        now: DateTime<Utc>,
    ) -> Result<(), sqlx::Error> {
        query!(tx, deactivate_all(now))
    }
}
