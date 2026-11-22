//! The accounts API: one sign-in for signing up and signing in, the
//! code and a new one, the last step, and sign out, see
//! `app_core::accounts` and `docs/design/accounts-sign-in.md`. A sign-in
//! answers 202 with the pending token, the code having been emailed;
//! the code answers whether the last step makes the account and asks
//! for its password, and that step answers the session and sets the
//! token as a cookie, which a browser sends from then on: only HTTP sees
//! it, only this site gets it, and only over TLS away from localhost; it
//! lasts until the browser closes, or, when the person chose to stay
//! signed in, as long as the token. A last step with a device's
//! challenge answers the device code instead and sets no cookie; the
//! device exchanges it with its verifier for its session, which it
//! keeps and sends as a bearer token. A sign-out deletes the token and
//! clears the cookie. Each route is rate limited, see `rate_limits`. A
//! sign-in from a trial user's browser may be theirs signing up, see
//! `trial_users`. A server whose settings turn sign-ups off,
//! `features.accounts_sign_up`, makes no account for a new email,
//! `sign_up_offered`. The accounts app's other routes are its
//! submodules, each at the path of the core module it calls.

pub mod app_store_notifications;
pub mod checkouts;
pub mod deletions;
pub mod password;
pub mod products;
pub mod purchases;
pub mod store_transactions;
pub mod stripe_events;
pub mod subscriptions;
pub mod trial_users;

use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum_extra::extract::CookieJar;
use axum_extra::extract::cookie::Cookie;
use serde::Deserialize;

use app_core::accounts::{self, CodeMatched, CodeSent, SecondFactor, Session};
use app_core::app::users;
use app_core::data::accounts_tokens::Context;
use app_core::data::app_users::User;
use app_core::deployment::Deployment;
use app_core::servers::settings::SettingsDocument;
use app_core::services::mailer::Mailer;

use crate::api_error::ApiError;
use crate::rate_limits::RateLimits;
use crate::user_auth::{CurrentToken, CurrentUser, TOKEN_COOKIE, signed_in_user_of, token_cookie};

#[derive(Deserialize)]
pub struct SignIn {
    pub email: String,
}

#[derive(Deserialize)]
pub struct Code {
    pub pending: String,
    pub code: String,
}

#[derive(Deserialize)]
pub struct Confirm {
    pub pending: String,
    /// The account's password, when it has one.
    pub password: Option<String>,
    /// A device's challenge, when the sign-in was started by one.
    pub challenge: Option<String>,
    /// Whether the browser stays signed in after it closes.
    #[serde(default)]
    pub keep_signed_in: bool,
}

#[derive(Deserialize)]
pub struct Password {
    pub pending: String,
    pub password: String,
    /// A device's challenge, when the sign-in was started by one.
    pub challenge: Option<String>,
    /// Whether the browser stays signed in after it closes.
    #[serde(default)]
    pub keep_signed_in: bool,
}

#[derive(Deserialize)]
pub struct DeviceCodeExchange {
    pub code: String,
    pub verifier: String,
}

/// Whether the server takes new sign-ups, trial users included; a 403
/// when its settings say not.
pub(crate) fn sign_up_offered(document: &SettingsDocument) -> Result<(), ApiError> {
    if document.settings.features.accounts_sign_up {
        Ok(())
    } else {
        Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "This server takes no new sign-ups",
        ))
    }
}

/// A sign-in, a sign-up for an email with no account when the server
/// takes them; from a trial user's browser, that trial user signing up,
/// who keeps what they made.
pub async fn sign_in(
    State(deployment): State<Deployment>,
    State(mailer): State<Mailer>,
    State(limits): State<RateLimits>,
    State(settings): State<Arc<SettingsDocument>>,
    jar: CookieJar,
    Json(sign_in): Json<SignIn>,
) -> Result<(StatusCode, Json<CodeSent>), ApiError> {
    limits.check_email(&sign_in.email)?;
    let offered = sign_up_offered(&settings).is_ok();
    let trial = if offered {
        trial_user_of(&deployment, &jar).await?
    } else {
        None
    };
    let sent = match trial {
        Some(trial) => {
            accounts::sign_in_from_trial(&deployment, &mailer, &trial, &sign_in.email).await?
        }
        None => accounts::sign_in(&deployment, &mailer, &sign_in.email, offered).await?,
    };
    Ok((StatusCode::ACCEPTED, Json(sent)))
}

/// The user of the browser's cookie, when they are a trial user.
async fn trial_user_of(deployment: &Deployment, jar: &CookieJar) -> Result<Option<User>, ApiError> {
    let Some(user) = signed_in_user_of(deployment, jar).await? else {
        return Ok(None);
    };
    let is_trial = app_core::accounts::trial_users::is_trial(deployment, &user)
        .await
        .map_err(users::UserError::from)?;
    Ok(is_trial.then_some(user))
}

#[derive(Deserialize)]
pub struct Resend {
    pub pending: String,
}

/// A new code emailed for the pending sign-in.
pub async fn code_resend(
    State(deployment): State<Deployment>,
    State(mailer): State<Mailer>,
    State(limits): State<RateLimits>,
    Json(resend): Json<Resend>,
) -> Result<StatusCode, ApiError> {
    limits.check_resend(&resend.pending)?;
    let email = accounts::resend_code_to(&deployment, &resend.pending).await?;
    limits.check_email(&email)?;
    accounts::resend_code(&deployment, &mailer, &resend.pending).await?;
    Ok(StatusCode::ACCEPTED)
}

/// The emailed code: whether the last step makes the account and asks
/// for its password, and its pending token. Nobody is signed in yet.
pub async fn code(
    State(deployment): State<Deployment>,
    Json(code): Json<Code>,
) -> Result<Json<CodeMatched>, ApiError> {
    let matched = accounts::sign_in_with_code(&deployment, &code.pending, &code.code).await?;
    Ok(Json(matched))
}

/// The last step, with the account's password when it has one: the
/// session with the cookie, or, with a device's challenge, the device
/// code and no cookie.
pub async fn sign_in_confirm(
    State(deployment): State<Deployment>,
    headers: HeaderMap,
    jar: CookieJar,
    Json(confirm): Json<Confirm>,
) -> Result<Response, ApiError> {
    let password = confirm.password.as_deref();
    if let Some(challenge) = challenge_of(&confirm.challenge) {
        let device_code = accounts::sign_in_confirm_for_device(
            &deployment,
            &confirm.pending,
            password,
            challenge,
        )
        .await?;
        return Ok(Json(device_code).into_response());
    }
    let session = accounts::sign_in_confirm(&deployment, &confirm.pending, password).await?;
    Ok(with_cookie(jar, &headers, session, confirm.keep_signed_in))
}

/// A new password in place of a forgotten one, after the code; 403
/// when the app does not let one be reset.
pub async fn sign_in_reset_password(
    State(deployment): State<Deployment>,
    State(mailer): State<Mailer>,
    headers: HeaderMap,
    jar: CookieJar,
    Json(password): Json<Password>,
) -> Result<Response, ApiError> {
    if let Some(challenge) = challenge_of(&password.challenge) {
        let device_code = accounts::sign_in_with_reset_password_for_device(
            &deployment,
            &mailer,
            &password.pending,
            &password.password,
            challenge,
        )
        .await?;
        return Ok(Json(device_code).into_response());
    }
    let session = accounts::sign_in_with_reset_password(
        &deployment,
        &mailer,
        &password.pending,
        &password.password,
    )
    .await?;
    Ok(with_cookie(jar, &headers, session, password.keep_signed_in))
}

/// `GET /api/accounts/second-factors`: what the user's account asks for
/// after the emailed code.
pub async fn second_factors(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
) -> Result<Json<Vec<SecondFactor>>, ApiError> {
    Ok(Json(accounts::second_factors(&deployment, &user).await?))
}

fn challenge_of(challenge: &Option<String>) -> Option<&str> {
    challenge
        .as_deref()
        .filter(|challenge| !challenge.is_empty())
}

fn with_cookie(
    jar: CookieJar,
    headers: &HeaderMap,
    session: Session,
    keep_signed_in: bool,
) -> Response {
    let keep_for =
        keep_signed_in.then(|| time::Duration::seconds(Context::Session.validity().num_seconds()));
    let jar = jar.add(token_cookie(&session.token, headers, keep_for));
    (jar, Json(session)).into_response()
}

/// A device's exchange of its code and verifier for its session; no
/// cookie, the device keeps the token.
pub async fn device_token(
    State(deployment): State<Deployment>,
    Json(exchange): Json<DeviceCodeExchange>,
) -> Result<Json<Session>, ApiError> {
    let session =
        accounts::sign_in_with_device_code(&deployment, &exchange.code, &exchange.verifier).await?;
    Ok(Json(session))
}

pub async fn sign_out(
    State(deployment): State<Deployment>,
    CurrentUser(_user): CurrentUser,
    CurrentToken(token): CurrentToken,
    jar: CookieJar,
) -> Result<(StatusCode, CookieJar), ApiError> {
    accounts::sign_out(&deployment, &token).await?;
    let jar = jar.remove(Cookie::build(TOKEN_COOKIE).path("/"));
    Ok((StatusCode::NO_CONTENT, jar))
}
