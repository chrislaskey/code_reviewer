//! Where the web page PUTs a file's bytes before it creates the file,
//! see `app_core::app::files::uploads`.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;

use app_core::app::files::uploads;
use app_core::deployment::Deployment;
use app_core::sync::bytes::{Upload, UploadNew};

use crate::api_error::ApiError;
use crate::user_auth::CurrentUser;

/// `POST /api/files/uploads`: 201 with where to PUT the bytes, or that
/// the bucket already has them.
pub async fn create(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
    Json(request): Json<UploadNew>,
) -> Result<(StatusCode, Json<Upload>), ApiError> {
    let upload = uploads::create(&deployment, &user, &request).await?;
    Ok((StatusCode::CREATED, Json(upload)))
}
