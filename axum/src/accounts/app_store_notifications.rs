//! App Store Server Notifications V2, posted by Apple with no session as
//! `{"signedPayload": "..."}`; Apple's signature is what is trusted, see
//! `app_core::accounts::app_store_notifications`. A server without the
//! App Store answers 404. A handled notification, or a repeat, answers
//! 200; anything else makes Apple send it again.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::Deserialize;

use app_core::accounts::app_store_notifications;
use app_core::deployment::Deployment;

use crate::api_error::ApiError;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Delivery {
    pub signed_payload: String,
}

/// `POST /api/accounts/app-store-notifications`.
pub async fn create(
    State(deployment): State<Deployment>,
    Json(delivery): Json<Delivery>,
) -> Result<StatusCode, ApiError> {
    let Some(app_store) = &deployment.app_store else {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "Not found"));
    };
    app_store_notifications::create(&deployment, app_store, &delivery.signed_payload).await?;
    Ok(StatusCode::OK)
}
