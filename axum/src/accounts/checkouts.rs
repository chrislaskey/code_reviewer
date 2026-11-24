//! The checkouts API, `app_core::accounts::checkouts` as JSON: buying one
//! price, which answers Stripe's page to send the person to, and the
//! checkout Stripe returns them to. Stripe's return links start with
//! `SERVER_URL`, the server's address as people reach it.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::Deserialize;
use uuid::Uuid;

use app_core::accounts::checkouts::{self, CheckoutCreated, CheckoutShown};
use app_core::accounts::online::ReturnTo;
use app_core::deployment::Deployment;
use app_core::services::stripe::Stripe;

use crate::api_error::ApiError;
use crate::config::ServerUrl;
use crate::user_auth::CurrentUser;

#[derive(Deserialize)]
pub struct CheckoutFields {
    pub price_id: Uuid,
    /// `app` when the desktop app asks, see `app_core::accounts::online`.
    #[serde(default)]
    pub return_to: ReturnTo,
}

/// `POST /api/accounts/checkouts`
pub async fn create(
    State(deployment): State<Deployment>,
    State(stripe): State<Option<Stripe>>,
    State(ServerUrl(server_url)): State<ServerUrl>,
    CurrentUser(user): CurrentUser,
    Json(fields): Json<CheckoutFields>,
) -> Result<(StatusCode, Json<CheckoutCreated>), ApiError> {
    let stripe = stripe.ok_or_else(no_stripe)?;
    let created = checkouts::create(
        &deployment,
        &stripe,
        &user,
        fields.price_id,
        &server_url,
        fields.return_to,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(created)))
}

/// `GET /api/accounts/checkouts/{id}`
pub async fn get(
    State(deployment): State<Deployment>,
    State(stripe): State<Option<Stripe>>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<Json<CheckoutShown>, ApiError> {
    let stripe = stripe.ok_or_else(no_stripe)?;
    Ok(Json(checkouts::get(&deployment, &stripe, &user, id).await?))
}

fn no_stripe() -> ApiError {
    ApiError::new(
        StatusCode::SERVICE_UNAVAILABLE,
        "This server does not take payments",
    )
}
