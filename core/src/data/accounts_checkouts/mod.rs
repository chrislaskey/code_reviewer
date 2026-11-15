//! Transaction-scoped checkouts records. Workflows own policy and commits.

use crate::accounts::checkouts::{AcceptedCheckout, Checkout};
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
) -> Result<Option<Checkout>, sqlx::Error> {
    Ok(query!(tx, get(id))?.filter(|row| row.user_id == user_id))
}

/// Persists a record accepted by the resource workflow.
pub(crate) async fn create(
    tx: &mut Transaction,
    checkout: AcceptedCheckout<'_>,
) -> Result<(), sqlx::Error> {
    query!(tx, insert(checkout.row()))
}

/// Persists a checkout status accepted by session processing.
pub(crate) async fn update(
    tx: &mut Transaction,
    id: Uuid,
    status: crate::accounts::checkouts::AcceptedStatus,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    query!(tx, update_status(id, status.value(), now))
}

/// Explicit processing access for trusted payment workflows.
pub(crate) mod processing {
    use super::*;

    /// Gets the checkout for a verified provider session.
    pub(crate) async fn get_by_stripe_session_id(
        tx: &mut Transaction,
        stripe_session_id: &str,
    ) -> Result<Option<Checkout>, sqlx::Error> {
        query!(tx, get_by_stripe_session_id(stripe_session_id))
    }

    /// Lists pending provider session IDs for reconciliation.
    pub(crate) async fn list_pending_stripe_session_ids(
        tx: &mut Transaction,
    ) -> Result<Vec<String>, sqlx::Error> {
        query!(tx, list_pending_stripe_session_ids())
    }
}
