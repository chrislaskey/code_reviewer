//! The daily job that catches what a missed App Store notification left
//! behind: every App Store subscription not ended asked of Apple again
//! and copied as it is now, as `stripe_reconcile` does for Stripe's.
//! Server only, on a server with the App Store.

use crate::data::accounts_subscriptions as accounts_subscriptions_records;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::accounts::app_store::{self, AppStore, AppStoreError};
use crate::accounts::purchases::Store;
use crate::accounts::store_transactions::{self, StoreTransactionError};
use crate::deployment::{Deployment, Role};

/// What the job asked Apple for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppStoreReconciled {
    pub subscriptions: usize,
    /// Apple has no such subscription, or it could not be written; logged.
    pub left: usize,
}

#[derive(Debug, Error)]
pub enum AppStoreReconcileError {
    #[error("only the server reconciles with the App Store")]
    ServerOnly,
    #[error(transparent)]
    AppStore(#[from] AppStoreError),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

pub async fn run(
    deployment: &Deployment,
    app_store: &AppStore,
) -> Result<AppStoreReconciled, AppStoreReconcileError> {
    if deployment.role != Role::Server {
        return Err(AppStoreReconcileError::ServerOnly);
    }
    let mut tx = deployment.begin().await?;
    let live =
        accounts_subscriptions_records::processing::list_live_store_ids(&mut tx, Store::AppStore)
            .await?;
    tx.commit().await?;

    let mut left = 0;
    for original_transaction_id in &live {
        let subscription = match app_store
            .subscription_get(original_transaction_id, None)
            .await
        {
            Ok(subscription) => subscription,
            Err(AppStoreError::NotFound(id)) => {
                tracing::warn!("app store subscription {id}: unknown to Apple, left");
                left += 1;
                continue;
            }
            Err(error) => return Err(error.into()),
        };
        let current = app_store::subscription_transaction(&subscription);
        let mut tx = deployment.begin().await?;
        match store_transactions::apply_in(&mut tx, &current, None).await {
            Ok(_) => {}
            Err(StoreTransactionError::Database(error)) => return Err(error.into()),
            Err(refused) => {
                tracing::warn!("app store subscription {original_transaction_id}: {refused}");
                left += 1;
            }
        }
        tx.commit().await?;
    }
    Ok(AppStoreReconciled {
        subscriptions: live.len(),
        left,
    })
}
