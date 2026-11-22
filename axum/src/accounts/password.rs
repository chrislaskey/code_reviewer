//! The password API: the signed-in user's password, their first or a
//! new one, see `app_core::accounts::password`.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;

use app_core::accounts::password::{self, PasswordUpdate};
use app_core::deployment::Deployment;

use crate::api_error::ApiError;
use crate::user_auth::CurrentUser;

/// `PUT /api/accounts/current/password`: the user's password, their first or a
/// new one.
pub async fn update(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
    Json(update): Json<PasswordUpdate>,
) -> Result<StatusCode, ApiError> {
    password::update(&deployment, &user, &update).await?;
    Ok(StatusCode::NO_CONTENT)
}
