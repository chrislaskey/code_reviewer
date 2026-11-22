//! The sync API: `app_core::sync`'s push, pull and backfill as JSON,
//! what a device calls to converge with this server, as the user its
//! token stands for, see `docs/design/offline-sync.md`. Every route of
//! it requires the device's protocol version, `protocol_version_required`,
//! and answers another version 426 until one side is updated; every
//! answer carries the server's settings version,
//! `settings_version_sent`, so a device fetches them again when they
//! changed.

pub mod bytes;

use std::sync::Arc;

use axum::Json;
use axum::extract::{Request, State};
use axum::http::{HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use app_core::deployment::Deployment;
use app_core::servers::settings::{SETTINGS_VERSION_HEADER, SettingsDocument};
use app_core::sync::{self, BackfillRequest, Page, PullRequest, PushRequest, PushResponse};

use crate::api_error::ApiError;
use crate::user_auth::CurrentUser;

/// The layer on the sync routes: a request with no protocol version, or
/// another than this server's, is refused 426, in words naming the side
/// to update, see `app_core::sync::protocol_version_refusal`.
pub async fn protocol_version_required(request: Request, next: Next) -> Response {
    let sent = request
        .headers()
        .get(sync::PROTOCOL_VERSION_HEADER)
        .and_then(|value| value.to_str().ok());
    match sync::protocol_version_refusal(sent) {
        Some(refusal) => ApiError::new(StatusCode::UPGRADE_REQUIRED, refusal).into_response(),
        None => next.run(request).await,
    }
}

/// The layer on the sync routes: every answer, a refusal too, carries
/// the server's settings version.
pub async fn settings_version_sent(
    State(document): State<Arc<SettingsDocument>>,
    request: Request,
    next: Next,
) -> Response {
    let mut response = next.run(request).await;
    if let Ok(version) = HeaderValue::from_str(&document.settings_version) {
        response
            .headers_mut()
            .insert(SETTINGS_VERSION_HEADER, version);
    }
    response
}

pub async fn push(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
    Json(request): Json<PushRequest>,
) -> Result<Json<PushResponse>, ApiError> {
    Ok(Json(sync::push(&deployment, &user, request).await?))
}

pub async fn pull(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
    Json(request): Json<PullRequest>,
) -> Result<Json<Page>, ApiError> {
    Ok(Json(sync::pull(&deployment, &user, request).await?))
}

pub async fn backfill(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
    Json(request): Json<BackfillRequest>,
) -> Result<Json<Page>, ApiError> {
    Ok(Json(sync::backfill(&deployment, &user, request).await?))
}
