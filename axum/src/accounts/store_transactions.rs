//! What the app bought in a store's sheet, see
//! `app_core::accounts::store_transactions`: the store's proof, checked
//! with the store and recorded for the signed-in user, answered as the
//! purchase or subscription it made, so the app finishes the store's
//! transaction only once this answered.

use axum::Json;
use axum::extract::State;
use serde_json::{Value, json};

use app_core::accounts::store_transactions::{self, StoreProof, StoreRow};
use app_core::accounts::{purchases, subscriptions};
use app_core::deployment::Deployment;

use crate::api_error::ApiError;
use crate::user_auth::CurrentUser;

/// `POST /api/accounts/store-transactions`, `{"store": "app_store",
/// "signed_transaction": "..."}`: `{"purchase": ...}` or
/// `{"subscription": ...}`.
pub async fn create(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
    Json(proof): Json<StoreProof>,
) -> Result<Json<Value>, ApiError> {
    let row = store_transactions::create(&deployment, &user, &proof).await?;
    Ok(Json(match row {
        StoreRow::Purchase(id) => {
            json!({ "purchase": purchases::get(&deployment, &user, id).await? })
        }
        StoreRow::Subscription(id) => {
            json!({ "subscription": subscriptions::get(&deployment, &user, id).await? })
        }
    }))
}
