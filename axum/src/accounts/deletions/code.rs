//! The code that confirms deleting an account, see
//! `app_core::accounts::deletions::code`.

use axum::extract::State;
use axum::http::StatusCode;

use app_core::accounts::deletions;
use app_core::deployment::Deployment;
use app_core::services::mailer::Mailer;

use crate::api_error::ApiError;
use crate::request_locale::RequestLocale;
use crate::user_auth::CurrentUser;

/// `POST /api/accounts/deletions/code`: mails the code, 204.
pub async fn create(
    State(deployment): State<Deployment>,
    State(mailer): State<Mailer>,
    CurrentUser(user): CurrentUser,
    RequestLocale(requested): RequestLocale,
) -> Result<StatusCode, ApiError> {
    deletions::code::create(&deployment, &mailer, &user, requested).await?;
    Ok(StatusCode::NO_CONTENT)
}
