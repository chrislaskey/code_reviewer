//! Where a device PUTs a synced row's bytes, see
//! `app_core::sync::bytes::uploads`.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;

use app_core::deployment::Deployment;
use app_core::sync::bytes::{Upload, UploadNew, uploads};

use crate::api_error::ApiError;
use crate::user_auth::CurrentUser;

/// `POST /api/sync/bytes/{table}/uploads`: 201 with where to PUT the
/// bytes, or that the bucket already has them.
pub async fn create(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
    Path(table): Path<String>,
    Json(request): Json<UploadNew>,
) -> Result<(StatusCode, Json<Upload>), ApiError> {
    let upload = uploads::create(&deployment, &user, &table, &request).await?;
    Ok((StatusCode::CREATED, Json(upload)))
}
