//! Stripe's Customer Portal for a plan's card, see
//! `app_core::accounts::subscriptions::portal`.

use axum::Json;
use axum::extract::{Path, State};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use app_core::accounts::online::ReturnTo;
use app_core::accounts::subscriptions;
use app_core::deployment::Deployment;
use app_core::services::stripe::Stripe;

use crate::api_error::ApiError;
use crate::config::ServerUrl;
use crate::user_auth::CurrentUser;

#[derive(Deserialize)]
pub struct PortalFields {
    /// `app` when the desktop app asks, see `app_core::accounts::online`.
    #[serde(default)]
    pub return_to: ReturnTo,
}

/// `POST /api/accounts/subscriptions/{id}/portal`: `{"url": ...}`, Stripe's
/// Customer Portal, which comes back to the plan's page.
pub async fn create(
    State(deployment): State<Deployment>,
    State(stripe): State<Option<Stripe>>,
    State(ServerUrl(server_url)): State<ServerUrl>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<Uuid>,
    fields: Option<Json<PortalFields>>,
) -> Result<Json<Value>, ApiError> {
    let stripe = stripe.ok_or_else(super::no_stripe)?;
    let return_to = fields
        .map(|Json(fields)| fields.return_to)
        .unwrap_or_default();
    let url =
        subscriptions::portal::create(&deployment, &stripe, &user, id, &server_url, return_to)
            .await?;
    Ok(Json(json!({ "url": url })))
}
