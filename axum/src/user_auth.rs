//! Who a request is from: the user its token stands for, see
//! `app_core::app::users::get_by_token`. The apps send the token as
//! `Authorization: Bearer`; a browser sends the `token` cookie a sign-in
//! set, see `accounts`. A request with no token, or one the server does
//! not know, is answered 401, whatever it asks for. The cookie is set
//! here, `token_cookie`, for the accounts routes that sign a browser in.

use axum::extract::{FromRef, FromRequestParts};
use axum::http::header::{AUTHORIZATION, HOST};
use axum::http::request::Parts;
use axum::http::{HeaderMap, StatusCode};
use axum_extra::extract::CookieJar;
use axum_extra::extract::cookie::{Cookie, SameSite};

use app_core::app::users;
use app_core::data::app_users::User;
use app_core::deployment::Deployment;

use crate::api_error::ApiError;

/// The cookie a browser keeps the token in.
pub const TOKEN_COOKIE: &str = "token";

/// The request's user, an extractor for every handler.
pub struct CurrentUser(pub User);

/// The token the request carried, for the sign-out to delete.
pub struct CurrentToken(pub String);

impl<S> FromRequestParts<S> for CurrentUser
where
    S: Send + Sync,
    Deployment: FromRef<S>,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<CurrentUser, ApiError> {
        let deployment = Deployment::from_ref(state);
        let token = token_of(parts)?;
        let user = users::get_by_token(&deployment, &token).await?;
        Ok(CurrentUser(user))
    }
}

impl<S> FromRequestParts<S> for CurrentToken
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<CurrentToken, ApiError> {
        token_of(parts).map(CurrentToken)
    }
}

/// The bearer token, else the cookie's.
fn token_of(parts: &Parts) -> Result<String, ApiError> {
    if let Some(header) = parts.headers.get(AUTHORIZATION) {
        let token = header
            .to_str()
            .ok()
            .and_then(|header| header.strip_prefix("Bearer "))
            .ok_or_else(|| ApiError::new(StatusCode::UNAUTHORIZED, "a bearer token is needed"))?;
        return Ok(token.trim().to_string());
    }
    CookieJar::from_headers(&parts.headers)
        .get(TOKEN_COOKIE)
        .map(|cookie| cookie.value().to_string())
        .ok_or_else(|| ApiError::new(StatusCode::UNAUTHORIZED, "Sign in first"))
}

/// The user of the browser's cookie, if it has one the server knows.
pub async fn signed_in_user_of(
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

/// The cookie a browser keeps the token in: for `keep_for`, or without
/// it until the browser closes. `Secure` unless the request came to
/// localhost, where there is no TLS.
pub fn token_cookie(
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

/// Whether the request came to this machine, where there is no TLS.
pub fn is_localhost(headers: &HeaderMap) -> bool {
    let host = headers
        .get(HOST)
        .and_then(|host| host.to_str().ok())
        .unwrap_or_default();
    let host = host.rsplit_once(':').map_or(host, |(name, _port)| name);
    matches!(host, "localhost" | "127.0.0.1" | "[::1]")
}
