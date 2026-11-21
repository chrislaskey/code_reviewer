//! Subscription records: the `Subscription` schema, its
//! `SubscriptionStatus`, and the reads and writes of
//! `accounts_subscriptions`. Callers own and commit the transaction.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::data::accounts_purchases::Store;
use crate::database::Transaction;

pub const TABLE: &str = "accounts_subscriptions";

/// Stripe's statuses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionStatus {
    Active,
    /// In its free days.
    Trialing,
    /// A renewal's payment failed, and Stripe is retrying it.
    PastDue,
    /// Stripe stopped retrying.
    Unpaid,
    /// The first payment is not made yet.
    Incomplete,
    IncompleteExpired,
    Paused,
    Canceled,
}

impl SubscriptionStatus {
    /// Its name in `accounts_subscriptions`.
    pub fn as_str(self) -> &'static str {
        match self {
            SubscriptionStatus::Active => "active",
            SubscriptionStatus::Trialing => "trialing",
            SubscriptionStatus::PastDue => "past_due",
            SubscriptionStatus::Unpaid => "unpaid",
            SubscriptionStatus::Incomplete => "incomplete",
            SubscriptionStatus::IncompleteExpired => "incomplete_expired",
            SubscriptionStatus::Paused => "paused",
            SubscriptionStatus::Canceled => "canceled",
        }
    }

    /// The status with this name, if there is one.
    pub fn from_name(name: &str) -> Option<SubscriptionStatus> {
        match name {
            "active" => Some(SubscriptionStatus::Active),
            "trialing" => Some(SubscriptionStatus::Trialing),
            "past_due" => Some(SubscriptionStatus::PastDue),
            "unpaid" => Some(SubscriptionStatus::Unpaid),
            "incomplete" => Some(SubscriptionStatus::Incomplete),
            "incomplete_expired" => Some(SubscriptionStatus::IncompleteExpired),
            "paused" => Some(SubscriptionStatus::Paused),
            "canceled" => Some(SubscriptionStatus::Canceled),
            _ => None,
        }
    }

    /// Whether it gives its plan: paid, or in its free days. A plan not
    /// paid is no plan.
    pub fn gives_plan(self) -> bool {
        matches!(
            self,
            SubscriptionStatus::Active | SubscriptionStatus::Trialing
        )
    }

    /// Whether a payment failed and the person can fix it with another card.
    pub fn is_payment_failed(self) -> bool {
        matches!(
            self,
            SubscriptionStatus::PastDue | SubscriptionStatus::Unpaid
        )
    }
}

/// A subscription of the account's, from either store.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Subscription {
    pub id: Uuid,
    pub user_id: Uuid,
    pub product_id: Uuid,
    pub price_id: Uuid,
    pub store: Store,
    /// The store's id, `sub_...` for Stripe.
    pub store_id: String,
    pub status: SubscriptionStatus,
    /// When the period paid for ends: the next renewal, or the end.
    pub current_period_end: Option<DateTime<Utc>>,
    pub cancel_at_period_end: bool,
    pub ended_at: Option<DateTime<Utc>>,
    /// The price a change waiting for the end of the period moves to.
    pub scheduled_price_id: Option<Uuid>,
    /// When it does.
    pub scheduled_at: Option<DateTime<Utc>>,
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
) -> Result<Option<(Subscription, String)>, sqlx::Error> {
    Ok(query!(tx, get(id))?.filter(|row| row.0.user_id == user_id))
}

/// Lists one account's records in their existing order.
pub async fn list(
    tx: &mut Transaction,
    user_id: Uuid,
) -> Result<Vec<(Subscription, String)>, sqlx::Error> {
    query!(tx, list_by_user(user_id))
}

/// Persists a record accepted by the resource workflow.
pub async fn upsert(tx: &mut Transaction, subscription: &Subscription) -> Result<(), sqlx::Error> {
    query!(tx, upsert(subscription))
}

/// Explicit processing access for trusted payment workflows.
pub mod processing {
    use super::*;

    /// Gets one stored record by ID.
    pub async fn get(
        tx: &mut Transaction,
        id: Uuid,
    ) -> Result<Option<(Subscription, String)>, sqlx::Error> {
        query!(tx, get(id))
    }

    /// Lists provider subscriptions that still need cancellation after account deletion.
    pub async fn list_live_store_ids_of_deleted_accounts(
        tx: &mut Transaction,
        store: Store,
    ) -> Result<Vec<String>, sqlx::Error> {
        query!(tx, list_live_store_ids_of_deleted_accounts(store))
    }

    /// Lists live provider subscriptions for reconciliation.
    pub async fn list_live_store_ids(
        tx: &mut Transaction,
        store: Store,
    ) -> Result<Vec<String>, sqlx::Error> {
        query!(tx, list_live_store_ids(store))
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
