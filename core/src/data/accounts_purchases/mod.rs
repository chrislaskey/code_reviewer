//! Purchase records: the `Purchase` schema, the `Store` it came from, and the
//! reads and writes of `accounts_purchases`. Callers own and commit the
//! transaction.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::database::Transaction;

pub const TABLE: &str = "accounts_purchases";

/// Who sold it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Store {
    Stripe,
    AppStore,
    PlayStore,
}

impl Store {
    /// Its name in `accounts_purchases`.
    pub fn as_str(self) -> &'static str {
        match self {
            Store::Stripe => "stripe",
            Store::AppStore => "app_store",
            Store::PlayStore => "play_store",
        }
    }

    /// The store with this name, if there is one.
    pub fn from_name(name: &str) -> Option<Store> {
        match name {
            "stripe" => Some(Store::Stripe),
            "app_store" => Some(Store::AppStore),
            "play_store" => Some(Store::PlayStore),
            _ => None,
        }
    }
}

/// A one-time purchase the account owns, from either store.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Purchase {
    pub id: Uuid,
    pub user_id: Uuid,
    pub product_id: Uuid,
    pub price_id: Uuid,
    /// The checkout that paid it; none for a phone store's.
    pub checkout_id: Option<Uuid>,
    pub store: Store,
    /// The store's id for the payment, `pi_...` for Stripe.
    pub store_id: String,
    /// What was paid, in the currency's smallest unit, tax included.
    pub amount: i64,
    pub currency: String,
    pub refunded_at: Option<DateTime<Utc>>,
    pub inserted_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A purchase as its pages show it, with what was bought.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PurchaseShown {
    #[serde(flatten)]
    pub purchase: Purchase,
    pub product_name: String,
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

/// Gets a record owned by the acting account; another account's is absent.
pub async fn get(
    tx: &mut Transaction,
    user_id: Uuid,
    id: Uuid,
) -> Result<Option<PurchaseShown>, sqlx::Error> {
    Ok(query!(tx, get(id))?.filter(|row| row.purchase.user_id == user_id))
}

/// Lists one account's records in their existing order.
pub async fn list(tx: &mut Transaction, user_id: Uuid) -> Result<Vec<PurchaseShown>, sqlx::Error> {
    query!(tx, list_by_user(user_id))
}

/// Records a store payment or delivery only when its unique key is new.
pub async fn create(tx: &mut Transaction, purchase: &Purchase) -> Result<(), sqlx::Error> {
    query!(tx, insert_if_missing(purchase))
}

/// Checks for an unrefunded purchase of a product by this account.
pub async fn owns(
    tx: &mut Transaction,
    user_id: Uuid,
    product_id: Uuid,
) -> Result<bool, sqlx::Error> {
    query!(tx, owns(user_id, product_id))
}

/// Marks a store payment refunded without deleting its history.
pub async fn refund(
    tx: &mut Transaction,
    store: Store,
    store_id: &str,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    query!(tx, refund(store, store_id, now))
}

/// Explicit processing access for trusted payment workflows.
pub mod processing {
    use super::*;

    /// Gets the purchase ID for a checkout.
    pub async fn get_by_checkout_id(
        tx: &mut Transaction,
        checkout_id: Uuid,
    ) -> Result<Option<Uuid>, sqlx::Error> {
        query!(tx, get_by_checkout_id(checkout_id))
    }

    /// Gets the record and owner IDs associated with one store payment.
    pub async fn get_ids_by_store_id(
        tx: &mut Transaction,
        store: Store,
        store_id: &str,
    ) -> Result<Option<(Uuid, Uuid)>, sqlx::Error> {
        query!(tx, get_ids_by_store_id(store, store_id))
    }
}
