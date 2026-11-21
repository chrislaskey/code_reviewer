//! Stripe's webhook, `POST /api/accounts/stripe-webhooks`: the events
//! Stripe sends, signed with `STRIPE_WEBHOOK_SECRET`, see
//! `app_core::accounts::stripe_events`. No session: the signature is
//! what proves Stripe sent it. Without the secret, or without Stripe,
//! `STRIPE_SECRET_KEY`, the route is a 404, as a path that does not
//! exist.

use std::env;
use std::sync::Arc;

use axum::Json;
use axum::extract::{FromRef, State};
use axum::http::{HeaderMap, StatusCode};
use secrecy::SecretString;
use serde_json::{Value, json};

use app_core::accounts::stripe_events::{self, StripeEventCreated};
use app_core::deployment::Deployment;
use app_core::services::stripe::Stripe;

use crate::api_error::ApiError;
use crate::router::AppState;

/// The webhook endpoint's signing secret, `whsec_...`, if the server has one.
#[derive(Clone, Default)]
pub struct StripeSigningSecret(pub Option<Arc<SecretString>>);

impl FromRef<AppState> for StripeSigningSecret {
    fn from_ref(state: &AppState) -> StripeSigningSecret {
        state.stripe_signing_secret.clone()
    }
}

/// `STRIPE_WEBHOOK_SECRET`, unset or blank for none.
pub fn open() -> StripeSigningSecret {
    let secret = env::var("STRIPE_WEBHOOK_SECRET")
        .ok()
        .filter(|secret| !secret.trim().is_empty())
        .map(|secret| Arc::new(SecretString::from(secret)));
    tracing::info!(
        "stripe webhook: {}",
        if secret.is_some() {
            "on"
        } else {
            "off, no STRIPE_WEBHOOK_SECRET"
        }
    );
    StripeSigningSecret(secret)
}

/// `POST /api/accounts/stripe-webhooks`: 200 for an event recorded or a
/// repeat, so Stripe stops sending it; 400 for a bad signature; 502
/// when Stripe could not be asked for the object, and 500 for any other
/// failure, which Stripe sends again.
pub async fn create(
    State(deployment): State<Deployment>,
    State(StripeSigningSecret(secret)): State<StripeSigningSecret>,
    State(stripe): State<Option<Stripe>>,
    headers: HeaderMap,
    body: String,
) -> Result<Json<Value>, ApiError> {
    let (Some(secret), Some(stripe)) = (secret, stripe) else {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "The server has no API route POST /api/accounts/stripe-webhooks",
        ));
    };
    let signature = headers
        .get("stripe-signature")
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| ApiError::new(StatusCode::BAD_REQUEST, "No Stripe-Signature header"))?;
    let created = stripe_events::create(&deployment, &stripe, &secret, &body, signature).await?;
    let repeat = matches!(created, StripeEventCreated::Repeat(_));
    Ok(Json(json!({ "received": true, "repeat": repeat })))
}
