//! Transaction-scoped subscriptions records. Workflows own policy and commits.

use crate::accounts::purchases::Store;
use crate::accounts::subscriptions::{AcceptedSubscription, Subscription};
use crate::database::Transaction;
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
) -> Result<Option<(Subscription, String)>, sqlx::Error> {
    Ok(query!(tx, get(id))?.filter(|row| row.0.user_id == user_id))
}

/// Lists one account's records in their existing order.
pub(crate) async fn list(
    tx: &mut Transaction,
    user_id: Uuid,
) -> Result<Vec<(Subscription, String)>, sqlx::Error> {
    query!(tx, list_by_user(user_id))
}

/// Persists a record accepted by the resource workflow.
pub(crate) async fn upsert(
    tx: &mut Transaction,
    subscription: AcceptedSubscription<'_>,
) -> Result<(), sqlx::Error> {
    query!(tx, upsert(subscription.row()))
}

/// Explicit processing access for trusted payment workflows.
pub(crate) mod processing {
    use super::*;

    /// Gets one stored record by ID.
    pub(crate) async fn get(
        tx: &mut Transaction,
        id: Uuid,
    ) -> Result<Option<(Subscription, String)>, sqlx::Error> {
        query!(tx, get(id))
    }

    /// Lists provider subscriptions that still need cancellation after account deletion.
    pub(crate) async fn list_live_store_ids_of_deleted_accounts(
        tx: &mut Transaction,
        store: Store,
    ) -> Result<Vec<String>, sqlx::Error> {
        query!(tx, list_live_store_ids_of_deleted_accounts(store))
    }

    /// Lists live provider subscriptions for reconciliation.
    pub(crate) async fn list_live_store_ids(
        tx: &mut Transaction,
        store: Store,
    ) -> Result<Vec<String>, sqlx::Error> {
        query!(tx, list_live_store_ids(store))
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
