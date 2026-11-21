//! Deleting the signed-in person's account, see
//! `app_core::accounts::deletions` and `docs/design/accounts-delete.md`:
//! a code mailed, then the delete with it, which clears the cookie. A
//! trial deletes with no code. Rate limited with the other accounts
//! routes, see `rate_limits`.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum_extra::extract::CookieJar;
use axum_extra::extract::cookie::Cookie;
use serde::Deserialize;

use app_core::accounts::deletions;
use app_core::deployment::Deployment;
use app_core::services::mailer::Mailer;

use crate::api_error::ApiError;
use crate::current_user::{CurrentUser, TOKEN_COOKIE};

#[derive(Deserialize)]
pub struct DeletionNew {
    /// The code mailed by `code_create`; none for a trial.
    pub code: Option<String>,
}

/// `POST /api/accounts/deletions/code`: mails the code, 204.
pub async fn code_create(
    State(deployment): State<Deployment>,
    State(mailer): State<Mailer>,
    CurrentUser(user): CurrentUser,
) -> Result<StatusCode, ApiError> {
    deletions::code_create(&deployment, &mailer, &user).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /api/accounts/deletions`: deletes the account, 204, and clears
/// the cookie.
pub async fn create(
    State(deployment): State<Deployment>,
    State(mailer): State<Mailer>,
    CurrentUser(user): CurrentUser,
    jar: CookieJar,
    Json(deletion): Json<DeletionNew>,
) -> Result<(StatusCode, CookieJar), ApiError> {
    deletions::create(&deployment, &mailer, &user, deletion.code.as_deref()).await?;
    let jar = jar.remove(Cookie::build(TOKEN_COOKIE).path("/"));
    Ok((StatusCode::NO_CONTENT, jar))
}
