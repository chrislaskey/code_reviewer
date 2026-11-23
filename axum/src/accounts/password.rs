//! The password API: whether the signed-in user has a password, and
//! their first or a new one, see `app_core::accounts::password`.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;

use app_core::accounts::password::{self, PasswordStatus, PasswordUpdate};
use app_core::deployment::Deployment;
use app_core::services::mailer::Mailer;

use crate::api_error::ApiError;
use crate::user_auth::CurrentUser;

/// `GET /api/accounts/current/password`: whether the user has a password.
pub async fn get(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
) -> Result<Json<PasswordStatus>, ApiError> {
    Ok(Json(password::get(&deployment, &user).await?))
}

/// `PUT /api/accounts/current/password`: the user's password, their first or a
/// new one, no current one asked; their email is told.
pub async fn update(
    State(deployment): State<Deployment>,
    State(mailer): State<Mailer>,
    CurrentUser(user): CurrentUser,
    Json(update): Json<PasswordUpdate>,
) -> Result<StatusCode, ApiError> {
    password::update(&deployment, &mailer, &user, &update).await?;
    Ok(StatusCode::NO_CONTENT)
}
