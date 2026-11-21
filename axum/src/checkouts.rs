//! The checkouts API, `app_core::accounts::checkouts` as JSON: buying one
//! price, which answers Stripe's page to send the person to, and the
//! checkout Stripe returns them to. Stripe's return links start with
//! `SERVER_URL`, the server's address as people reach it.

use axum::Json;
use axum::extract::{FromRef, Path, State};
use axum::http::StatusCode;
use serde::Deserialize;
use uuid::Uuid;

use app_core::accounts::checkouts::{self, CheckoutCreated, CheckoutShown};
use app_core::accounts::online::ReturnTo;
use app_core::deployment::Deployment;
use app_core::services::stripe::Stripe;

use crate::api_error::ApiError;
use crate::current_user::CurrentUser;
use crate::router::AppState;

/// The server's address as people reach it, with no trailing slash,
/// `SERVER_URL`, `http://localhost:4000` when unset.
#[derive(Clone)]
pub struct ServerUrl(pub String);

impl FromRef<AppState> for ServerUrl {
    fn from_ref(state: &AppState) -> ServerUrl {
        state.server_url.clone()
    }
}

impl ServerUrl {
    pub fn open() -> Result<ServerUrl, String> {
        let url =
            std::env::var("SERVER_URL").unwrap_or_else(|_| "http://localhost:4000".to_string());
        let url = url.trim().trim_end_matches('/').to_string();
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(format!(
                "SERVER_URL must start with http:// or https://, got '{url}'"
            ));
        }
        Ok(ServerUrl(url))
    }
}

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
