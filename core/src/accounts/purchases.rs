//! One-time purchases, one row of `accounts_purchases` each, written when
//! a checkout is paid, see `checkouts`. Server only. A purchase records
//! the store that sold it, Stripe now, the phone stores later, and the
//! store's id for it, which is how a repeat of the same payment is
//! recognized. A refunded purchase gives nothing.

use crate::data::accounts_purchases::{self, Purchase, PurchaseShown, Store};
use chrono::{DateTime, Utc};
use thiserror::Error;
use uuid::Uuid;

use crate::data::app_users::User;
use crate::database::Transaction;
use crate::deployment::{Deployment, Role};

#[derive(Debug, Error)]
pub enum PurchaseError {
    #[error("only the server has purchases")]
    ServerOnly,
    #[error("No purchase {id} of yours")]
    NotFound { id: Uuid },
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// The user's purchases, newest first, refunded ones too.
pub async fn list(
    deployment: &Deployment,
    user: &User,
) -> Result<Vec<PurchaseShown>, PurchaseError> {
    if deployment.role != Role::Server {
        return Err(PurchaseError::ServerOnly);
    }
    let mut tx = deployment.begin().await?;
    let purchases = accounts_purchases::list(&mut tx, user.id).await?;
    tx.commit().await?;
    Ok(purchases)
}

/// One of the user's purchases.
pub async fn get(
    deployment: &Deployment,
    user: &User,
    id: Uuid,
) -> Result<PurchaseShown, PurchaseError> {
    if deployment.role != Role::Server {
        return Err(PurchaseError::ServerOnly);
    }
    let mut tx = deployment.begin().await?;
    let purchase = accounts_purchases::get(&mut tx, user.id, id).await?;
    tx.commit().await?;
    purchase.ok_or(PurchaseError::NotFound { id })
}

/// Records the purchase, unless the store's payment is recorded already.
pub(crate) async fn insert_if_missing_in(
    tx: &mut Transaction,
    purchase: &Purchase,
) -> Result<(), sqlx::Error> {
    accounts_purchases::create(&mut *tx, purchase).await
}

/// Whether the user has the product, bought and not refunded.
pub(crate) async fn owns_in(
    tx: &mut Transaction,
    user_id: Uuid,
    product_id: Uuid,
) -> Result<bool, sqlx::Error> {
    accounts_purchases::owns(&mut *tx, user_id, product_id).await
}

/// The store's payment refunded in full: the purchase gives nothing now.
pub(crate) async fn refund_in(
    tx: &mut Transaction,
    store: Store,
    store_id: &str,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    accounts_purchases::refund(&mut *tx, store, store_id, now).await
}
