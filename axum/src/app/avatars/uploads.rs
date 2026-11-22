//! Where the person PUTs the picture they picked, see
//! `app_core::app::avatars::uploads`.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;

use app_core::app::avatars::uploads;
use app_core::app::avatars::{AvatarUpload, AvatarUploadNew};
use app_core::deployment::Deployment;

use crate::api_error::ApiError;
use crate::user_auth::CurrentUser;

/// `POST /api/users/current/avatar/uploads`: 201 with where to PUT the
/// picture.
pub async fn create(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
    Json(request): Json<AvatarUploadNew>,
) -> Result<(StatusCode, Json<AvatarUpload>), ApiError> {
    let upload = uploads::create(&deployment, &user, &request).await?;
    Ok((StatusCode::CREATED, Json(upload)))
}
