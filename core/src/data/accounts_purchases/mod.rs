//! Transaction-scoped purchases records. Workflows own policy and commits.

use crate::accounts::purchases::{AcceptedPurchase, PurchaseShown, Store};
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

/// Gets a record owned by the acting account; another account's is absent.
pub(crate) async fn get(
    tx: &mut Transaction,
    user_id: Uuid,
    id: Uuid,
) -> Result<Option<PurchaseShown>, sqlx::Error> {
    Ok(query!(tx, get(id))?.filter(|row| row.purchase.user_id == user_id))
}

/// Lists one account's records in their existing order.
pub(crate) async fn list(
    tx: &mut Transaction,
    user_id: Uuid,
) -> Result<Vec<PurchaseShown>, sqlx::Error> {
    query!(tx, list_by_user(user_id))
}

/// Records a store payment or delivery only when its unique key is new.
pub(crate) async fn create(
    tx: &mut Transaction,
    purchase: AcceptedPurchase<'_>,
) -> Result<(), sqlx::Error> {
    query!(tx, insert_if_missing(purchase.row()))
}

/// Checks for an unrefunded purchase of a product by this account.
pub(crate) async fn owns(
    tx: &mut Transaction,
    user_id: Uuid,
    product_id: Uuid,
) -> Result<bool, sqlx::Error> {
    query!(tx, owns(user_id, product_id))
}

/// Marks a store payment refunded without deleting its history.
pub(crate) async fn refund(
    tx: &mut Transaction,
    store: Store,
    store_id: &str,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    query!(tx, refund(store, store_id, now))
}

/// Explicit processing access for trusted payment workflows.
pub(crate) mod processing {
    use super::*;

    /// Gets the purchase ID for a checkout.
    pub(crate) async fn get_by_checkout_id(
        tx: &mut Transaction,
        checkout_id: Uuid,
    ) -> Result<Option<Uuid>, sqlx::Error> {
        query!(tx, get_by_checkout_id(checkout_id))
    }

    /// Gets the record and owner IDs associated with one store payment.
    pub(crate) async fn get_ids_by_store_id(
        tx: &mut Transaction,
        store: Store,
        store_id: &str,
    ) -> Result<Option<(Uuid, Uuid)>, sqlx::Error> {
        query!(tx, get_ids_by_store_id(store, store_id))
    }
}
