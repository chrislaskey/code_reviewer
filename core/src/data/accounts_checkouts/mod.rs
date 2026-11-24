//! Checkout records: the `Checkout` schema, its `CheckoutStatus`, and the
//! reads and writes of `accounts_checkouts`. Callers own and commit the
//! transaction.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::database::Transaction;

pub const TABLE: &str = "accounts_checkouts";

/// Where a checkout stands with Stripe.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckoutStatus {
    /// On Stripe's page, not paid yet.
    Open,
    /// Paid with a method that settles later.
    Processing,
    Paid,
    /// Left unpaid until Stripe closed it.
    Expired,
    /// A payment that settles later and did not.
    Failed,
}

impl CheckoutStatus {
    /// Its name in `accounts_checkouts`.
    pub fn as_str(self) -> &'static str {
        match self {
            CheckoutStatus::Open => "open",
            CheckoutStatus::Processing => "processing",
            CheckoutStatus::Paid => "paid",
            CheckoutStatus::Expired => "expired",
            CheckoutStatus::Failed => "failed",
        }
    }

    /// The status with this name, if there is one.
    pub fn from_name(name: &str) -> Option<CheckoutStatus> {
        match name {
            "open" => Some(CheckoutStatus::Open),
            "processing" => Some(CheckoutStatus::Processing),
            "paid" => Some(CheckoutStatus::Paid),
            "expired" => Some(CheckoutStatus::Expired),
            "failed" => Some(CheckoutStatus::Failed),
            _ => None,
        }
    }
}

/// A Stripe Checkout session the account started, as the server keeps it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checkout {
    pub id: Uuid,
    pub user_id: Uuid,
    pub price_id: Uuid,
    /// `cs_...`
    pub stripe_session_id: String,
    pub status: CheckoutStatus,
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

/// Gets a record owned by the acting account; another account's is absent.
pub async fn get(
    tx: &mut Transaction,
    user_id: Uuid,
    id: Uuid,
) -> Result<Option<Checkout>, sqlx::Error> {
    Ok(query!(tx, get(id))?.filter(|row| row.user_id == user_id))
}

/// Persists a record accepted by the resource workflow.
pub async fn create(tx: &mut Transaction, checkout: &Checkout) -> Result<(), sqlx::Error> {
    query!(tx, insert(checkout))
}

/// Persists a checkout status accepted by session processing.
pub async fn update(
    tx: &mut Transaction,
    id: Uuid,
    status: CheckoutStatus,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    query!(tx, update_status(id, status, now))
}

/// Explicit processing access for trusted payment workflows.
pub mod processing {
    use super::*;

    /// Gets the checkout for a verified provider session.
    pub async fn get_by_stripe_session_id(
        tx: &mut Transaction,
        stripe_session_id: &str,
    ) -> Result<Option<Checkout>, sqlx::Error> {
        query!(tx, get_by_stripe_session_id(stripe_session_id))
    }

    /// Lists pending provider session IDs for reconciliation.
    pub async fn list_pending_stripe_session_ids(
        tx: &mut Transaction,
    ) -> Result<Vec<String>, sqlx::Error> {
        query!(tx, list_pending_stripe_session_ids())
    }
}
