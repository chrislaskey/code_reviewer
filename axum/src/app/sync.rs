//! The sync API: `app_core::app::sync`'s push, pull and backfill as JSON,
//! what a device calls to converge with this server, as the user its
//! token stands for, see `docs/design/offline-sync.md`.

use axum::Json;
use axum::extract::State;

use app_core::app::sync::{self, BackfillRequest, Page, PullRequest, PushRequest, PushResponse};
use app_core::deployment::Deployment;

use crate::api_error::ApiError;
use crate::user_auth::CurrentUser;

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
