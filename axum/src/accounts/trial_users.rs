//! "Try it now": a trial user with no sign-up, see
//! `app_core::accounts::trial_users`, whose cookie lasts until their rows
//! go; a sign-up from that browser is theirs, see `accounts::sign_in`.
//! None on a server that takes no new sign-ups.

use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum_extra::extract::CookieJar;
use serde::Serialize;

use app_core::accounts::AccountError;
use app_core::accounts::trial_users;
use app_core::deployment::Deployment;
use app_core::servers::settings::SettingsDocument;

use super::sign_up_enabled;
use crate::api_error::ApiError;
use crate::config::TrialUsersMax;
use crate::rate_limits::{ClientIpAddr, RateLimits};
use crate::user_auth::{signed_in_user_of, token_cookie};

#[derive(Serialize)]
pub struct TrialAvailable {
    pub available: bool,
}

/// `GET /api/accounts/trial`: whether "Try it now" would make a trial
/// user, for the pages to offer it.
pub async fn get(
    State(deployment): State<Deployment>,
    State(TrialUsersMax(max)): State<TrialUsersMax>,
    State(settings): State<Arc<SettingsDocument>>,
) -> Result<Json<TrialAvailable>, ApiError> {
    let count = trial_users::count(&deployment)
        .await
        .map_err(AccountError::from)?;
    let available = sign_up_enabled(&settings).is_ok() && max > 0 && count < max;
    Ok(Json(TrialAvailable { available }))
}

/// `POST /api/accounts/trial`, "Try it now": a trial user, signed in
/// with a cookie that lasts as long as their rows are kept. A browser
/// already signed in, as a trial user or anyone, is answered who it is
/// and nothing is made. A client IP makes so many an hour, see
/// `rate_limits`; one refused because the server holds all the trial
/// users it may is a 503, which `ApiError` writes to the log.
pub async fn create(
    State(deployment): State<Deployment>,
    State(TrialUsersMax(max)): State<TrialUsersMax>,
    State(limits): State<RateLimits>,
    State(settings): State<Arc<SettingsDocument>>,
    ClientIpAddr(client_ip): ClientIpAddr,
    headers: HeaderMap,
    jar: CookieJar,
) -> Result<Response, ApiError> {
    if signed_in_user_of(&deployment, &jar).await?.is_some() {
        return Ok(StatusCode::NO_CONTENT.into_response());
    }
    sign_up_enabled(&settings)?;
    limits.check_trial_user(client_ip)?;
    let session = trial_users::create(&deployment, max)
        .await
        .map_err(AccountError::from)?;
    let kept_for = time::Duration::seconds(trial_users::kept_for().num_seconds());
    let jar = jar.add(token_cookie(&session.token, &headers, Some(kept_for)));
    Ok((StatusCode::CREATED, jar, Json(session)).into_response())
}
