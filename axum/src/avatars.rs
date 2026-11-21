//! The avatars API: `app_core::app::avatars` as JSON, for the request's user.
//! `POST /api/avatars/uploads` answers where to PUT the picture, and
//! `PUT /api/avatars` makes the uploaded file the user's picture. Both
//! answer 503 on a server with no bucket, see `storage`.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;

use app_core::app::avatars::{self, AvatarUpdate, AvatarUpload, AvatarUploadNew};
use app_core::data::app_users::User;
use app_core::deployment::Deployment;

use crate::api_error::ApiError;
use crate::current_user::CurrentUser;

pub async fn create_upload(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
    Json(request): Json<AvatarUploadNew>,
) -> Result<(StatusCode, Json<AvatarUpload>), ApiError> {
    let upload = avatars::create_upload(&deployment, &user, &request).await?;
    Ok((StatusCode::CREATED, Json(upload)))
}

pub async fn update(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
    Json(request): Json<AvatarUpdate>,
) -> Result<Json<User>, ApiError> {
    Ok(Json(avatars::update(&deployment, &user, &request).await?))
}
