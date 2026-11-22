//! The subscriptions API, `app_core::accounts::subscriptions` as JSON:
//! the request's user's plans, the current one and the ended ones, the
//! changes they make to one, and Stripe's Customer Portal for its card.

pub mod portal;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use uuid::Uuid;

use app_core::accounts::subscriptions::{self, SubscriptionChange, SubscriptionShown};
use app_core::deployment::Deployment;
use app_core::services::stripe::Stripe;

use crate::api_error::ApiError;
use crate::user_auth::CurrentUser;

/// `GET /api/accounts/subscriptions`
pub async fn list(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
) -> Result<Json<Vec<SubscriptionShown>>, ApiError> {
    Ok(Json(subscriptions::list(&deployment, &user).await?))
}

/// `GET /api/accounts/subscriptions/{id}`
pub async fn get(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<Json<SubscriptionShown>, ApiError> {
    Ok(Json(subscriptions::get(&deployment, &user, id).await?))
}

/// `PUT /api/accounts/subscriptions/{id}`, with the change:
/// `{"action": "switch", "price_id": ...}`, `{"action": "switch_undo"}`,
/// `{"action": "cancel"}` or `{"action": "resume"}`.
pub async fn update(
    State(deployment): State<Deployment>,
    State(stripe): State<Option<Stripe>>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<Uuid>,
    Json(change): Json<SubscriptionChange>,
) -> Result<Json<SubscriptionShown>, ApiError> {
    let stripe = stripe.ok_or_else(no_stripe)?;
    Ok(Json(
        subscriptions::update(&deployment, &stripe, &user, id, change).await?,
    ))
}

fn no_stripe() -> ApiError {
    ApiError::new(
        StatusCode::SERVICE_UNAVAILABLE,
        "This server does not take payments",
    )
}
