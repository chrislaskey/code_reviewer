//! The avatars API: `app_core::app::avatars` as JSON, for the request's user.
//! `POST /api/users/current/avatar/uploads`, `uploads`, answers where to PUT the picture, and
//! `PUT /api/users/current/avatar` makes the uploaded file the user's picture. Both
//! answer 503 on a server with no bucket, see `config::storage`.

pub mod uploads;

use axum::Json;
use axum::extract::State;

use app_core::app::avatars::{self, AvatarUpdate};
use app_core::data::app_users::User;
use app_core::deployment::Deployment;

use crate::api_error::ApiError;
use crate::user_auth::CurrentUser;

pub async fn update(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
    Json(request): Json<AvatarUpdate>,
) -> Result<Json<User>, ApiError> {
    Ok(Json(avatars::update(&deployment, &user, &request).await?))
}
