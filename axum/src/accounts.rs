//! The accounts API: sign up, sign in, the code and a new one, the
//! password after it, and sign out, see `app_core::accounts` and
//! `docs/design/accounts-sign-in.md`. A sign-up or a sign-in answers 202
//! with the pending token, the code having been emailed; the code
//! answers the session and sets the token as a cookie, which a browser
//! sends from then on: only HTTP sees it, only this site gets it, and
//! only over TLS away from localhost; it lasts until the browser closes,
//! or, when the person chose to stay signed in, as long as the token. When the account has a password,
//! or is new and may make one, the code answers that next step instead,
//! and the password finishes as the code would have. A step with a
//! device's challenge answers the device code instead and sets no
//! cookie; the device exchanges it with its verifier for its session,
//! which it keeps and sends as a bearer token. A sign-out deletes the
//! token and clears the cookie. Each route is rate limited, see
//! `rate_limits`. "Try it now" makes a trial user with no sign-up, see
//! `app_core::accounts::trial_users`, whose cookie lasts until their rows go; a
//! sign-up from that browser is theirs.

use axum::Json;
use axum::extract::State;
use axum::http::header::HOST;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum_extra::extract::CookieJar;
use axum_extra::extract::cookie::{Cookie, SameSite};
use serde::{Deserialize, Serialize};

use app_core::accounts::trial_users;
use app_core::accounts::{
    self, AccountError, CodeSent, PasswordUpdate, SecondFactor, Session, SignInStep,
};
use app_core::app::users::{self, User};
use app_core::deployment::Deployment;
use app_core::services::mailer::Mailer;
use app_core::tokens::Context;

use crate::api_error::ApiError;
use crate::current_user::{CurrentToken, CurrentUser, TOKEN_COOKIE};
use crate::rate_limits::{ClientIpAddr, RateLimits};
use crate::router::TrialUsersMax;

#[derive(Deserialize)]
pub struct SignIn {
    pub email: String,
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
pub struct NewPassword {
    pub pending: String,
    /// None when the person skipped making one.
    pub password: Option<String>,
    /// A device's challenge, when the sign-in was started by one.
    pub challenge: Option<String>,
    /// Whether the browser stays signed in after it closes.
    #[serde(default)]
    pub keep_signed_in: bool,
}

#[derive(Deserialize)]
pub struct Code {
    pub pending: String,
    pub code: String,
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

#[derive(Deserialize)]
pub struct SignUp {
    pub email: String,
}

/// A sign-up; from a trial user's browser, that trial user signing up,
/// who keeps what they made.
pub async fn sign_up(
    State(deployment): State<Deployment>,
    State(mailer): State<Mailer>,
    State(limits): State<RateLimits>,
    jar: CookieJar,
    Json(sign_up): Json<SignUp>,
) -> Result<(StatusCode, Json<CodeSent>), ApiError> {
    limits.check_email(&sign_up.email)?;
    let sent = match trial_user_of(&deployment, &jar).await? {
        Some(trial) => {
            accounts::sign_up_from_trial(&deployment, &mailer, &trial, &sign_up.email).await?
        }
        None => accounts::sign_up(&deployment, &mailer, &sign_up.email).await?,
    };
    Ok((StatusCode::ACCEPTED, Json(sent)))
}

/// The user of the browser's cookie, when they are a trial user.
async fn trial_user_of(deployment: &Deployment, jar: &CookieJar) -> Result<Option<User>, ApiError> {
    let Some(user) = signed_in_user_of(deployment, jar).await? else {
        return Ok(None);
    };
    let is_trial = trial_users::is_trial(deployment, &user)
        .await
        .map_err(users::UserError::from)?;
    Ok(is_trial.then_some(user))
}

/// The user of the browser's cookie, if it has one the server knows.
async fn signed_in_user_of(
    deployment: &Deployment,
    jar: &CookieJar,
) -> Result<Option<User>, ApiError> {
    let Some(cookie) = jar.get(TOKEN_COOKIE) else {
        return Ok(None);
    };
    match users::get_by_token(deployment, cookie.value()).await {
        Ok(user) => Ok(Some(user)),
        Err(users::UserError::UnknownToken | users::UserError::AccountDeleted) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

#[derive(Serialize)]
pub struct TrialAvailable {
    pub available: bool,
}

/// `GET /api/accounts/trial`: whether "Try it now" would make a trial
/// user, for the pages to offer it.
pub async fn trial_get(
    State(deployment): State<Deployment>,
    State(TrialUsersMax(max)): State<TrialUsersMax>,
) -> Result<Json<TrialAvailable>, ApiError> {
    let count = trial_users::count(&deployment)
        .await
        .map_err(AccountError::from)?;
    let available = max > 0 && count < max;
    Ok(Json(TrialAvailable { available }))
}

/// `POST /api/accounts/trial`, "Try it now": a trial user, signed in
/// with a cookie that lasts as long as their rows are kept. A browser
/// already signed in, as a trial user or anyone, is answered who it is
/// and nothing is made. A client IP makes so many an hour, see
/// `rate_limits`; one refused because the server holds all the trial
/// users it may is a 503, which `ApiError` writes to the log.
pub async fn trial_create(
    State(deployment): State<Deployment>,
    State(TrialUsersMax(max)): State<TrialUsersMax>,
    State(limits): State<RateLimits>,
    ClientIpAddr(client_ip): ClientIpAddr,
    headers: HeaderMap,
    jar: CookieJar,
) -> Result<Response, ApiError> {
    if signed_in_user_of(&deployment, &jar).await?.is_some() {
        return Ok(StatusCode::NO_CONTENT.into_response());
    }
    limits.check_trial_user(client_ip)?;
    let session = trial_users::create(&deployment, max)
        .await
        .map_err(AccountError::from)?;
    let kept_for = time::Duration::seconds(trial_users::kept_for().num_seconds());
    let jar = jar.add(token_cookie(&session.token, &headers, Some(kept_for)));
    Ok((StatusCode::CREATED, jar, Json(session)).into_response())
}

pub async fn sign_in(
    State(deployment): State<Deployment>,
    State(mailer): State<Mailer>,
    State(limits): State<RateLimits>,
    Json(sign_in): Json<SignIn>,
) -> Result<(StatusCode, Json<CodeSent>), ApiError> {
    limits.check_email(&sign_in.email)?;
    let sent = accounts::sign_in(&deployment, &mailer, &sign_in.email).await?;
    Ok((StatusCode::ACCEPTED, Json(sent)))
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

/// The emailed code: the session with the cookie, or, with a device's
/// challenge, the device code and no cookie; or the step still to take.
pub async fn code(
    State(deployment): State<Deployment>,
    headers: HeaderMap,
    jar: CookieJar,
    Json(code): Json<Code>,
) -> Result<Response, ApiError> {
    if let Some(challenge) = challenge_of(&code.challenge) {
        let step = accounts::sign_in_with_code_for_device(
            &deployment,
            &code.pending,
            &code.code,
            challenge,
        )
        .await?;
        return Ok(Json(step).into_response());
    }
    match accounts::sign_in_with_code(&deployment, &code.pending, &code.code).await? {
        SignInStep::Finished(session) => {
            Ok(with_cookie(jar, &headers, session, code.keep_signed_in))
        }
        next => Ok(Json(next).into_response()),
    }
}

/// The account's password, after the code: answers as the code does
/// when nothing else is asked.
pub async fn sign_in_password(
    State(deployment): State<Deployment>,
    headers: HeaderMap,
    jar: CookieJar,
    Json(password): Json<Password>,
) -> Result<Response, ApiError> {
    if let Some(challenge) = challenge_of(&password.challenge) {
        let device_code = accounts::sign_in_with_password_for_device(
            &deployment,
            &password.pending,
            &password.password,
            challenge,
        )
        .await?;
        return Ok(Json(device_code).into_response());
    }
    let session =
        accounts::sign_in_with_password(&deployment, &password.pending, &password.password).await?;
    Ok(with_cookie(jar, &headers, session, password.keep_signed_in))
}

/// The password a new account makes, or skips, after its first code.
pub async fn sign_in_new_password(
    State(deployment): State<Deployment>,
    headers: HeaderMap,
    jar: CookieJar,
    Json(new): Json<NewPassword>,
) -> Result<Response, ApiError> {
    let password = new.password.as_deref();
    if let Some(challenge) = challenge_of(&new.challenge) {
        let device_code = accounts::sign_in_with_new_password_for_device(
            &deployment,
            &new.pending,
            password,
            challenge,
        )
        .await?;
        return Ok(Json(device_code).into_response());
    }
    let session = accounts::sign_in_with_new_password(&deployment, &new.pending, password).await?;
    Ok(with_cookie(jar, &headers, session, new.keep_signed_in))
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

/// `PUT /api/accounts/password`: the user's password, their first or a
/// new one.
pub async fn password_update(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
    Json(update): Json<PasswordUpdate>,
) -> Result<StatusCode, ApiError> {
    accounts::password_update(&deployment, &user, &update).await?;
    Ok(StatusCode::NO_CONTENT)
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

/// The cookie a browser keeps the token in: for `keep_for`, or without
/// it until the browser closes. `Secure` unless the request came to
/// localhost, where there is no TLS.
pub(crate) fn token_cookie(
    token: &str,
    headers: &HeaderMap,
    keep_for: Option<time::Duration>,
) -> Cookie<'static> {
    let cookie = Cookie::build((TOKEN_COOKIE, token.to_string()))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Strict)
        .secure(!is_localhost(headers));
    match keep_for {
        Some(keep_for) => cookie.max_age(keep_for).build(),
        None => cookie.build(),
    }
}

pub(crate) fn is_localhost(headers: &HeaderMap) -> bool {
    let host = headers
        .get(HOST)
        .and_then(|host| host.to_str().ok())
        .unwrap_or_default();
    let host = host.rsplit_once(':').map_or(host, |(name, _port)| name);
    matches!(host, "localhost" | "127.0.0.1" | "[::1]")
}
