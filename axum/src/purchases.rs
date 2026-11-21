//! The purchases API, `app_core::accounts::purchases` as JSON: the
//! request's user's one-time purchases.

use axum::Json;
use axum::extract::{Path, State};
use uuid::Uuid;

use app_core::accounts::purchases;
use app_core::data::accounts_purchases::PurchaseShown;
use app_core::deployment::Deployment;

use crate::api_error::ApiError;
use crate::current_user::CurrentUser;

/// `GET /api/accounts/purchases`
pub async fn list(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
) -> Result<Json<Vec<PurchaseShown>>, ApiError> {
    Ok(Json(purchases::list(&deployment, &user).await?))
}

/// `GET /api/accounts/purchases/{id}`
pub async fn get(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<Json<PurchaseShown>, ApiError> {
    Ok(Json(purchases::get(&deployment, &user, id).await?))
}
