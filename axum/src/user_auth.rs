//! Who a request is from: the user its token stands for, see
//! `app_core::app::users::get_by_token`. The apps send the token as
//! `Authorization: Bearer`; a browser sends the `token` cookie a sign-in
//! set, see `accounts`. A request with no token, or one the server does
//! not know, is answered 401, whatever it asks for. The cookie is set
//! here, `token_cookie`, for the accounts routes that sign a browser in.
//! A route a page must not reach just by being open in the browser, as
//! the browser sends the cookie whichever page asks, takes
//! `CurrentUserFromBearerToken`, which never reads the cookie; one for
//! a browser alone takes `CurrentUserFromCookie`.

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

/// The request's user by its bearer token alone, the cookie left unread:
/// for a route only code holding the token may call, not any page open
/// in a browser signed in to this server.
#[allow(
    dead_code,
    reason = "for an app's routes; the template's all take the cookie too"
)]
pub struct CurrentUserFromBearerToken(pub User);

/// The request's user by the browser's cookie alone, any bearer token
/// left unread: for a route only a browser signed in to this server calls.
#[allow(
    dead_code,
    reason = "for an app's routes; the template's all take the bearer token too"
)]
pub struct CurrentUserFromCookie(pub User);

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

impl<S> FromRequestParts<S> for CurrentUserFromBearerToken
where
    S: Send + Sync,
    Deployment: FromRef<S>,
{
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> Result<CurrentUserFromBearerToken, ApiError> {
        let deployment = Deployment::from_ref(state);
        let token = bearer_token_of(parts)?.ok_or_else(sign_in_first)?;
        let user = users::get_by_token(&deployment, &token).await?;
        Ok(CurrentUserFromBearerToken(user))
    }
}

impl<S> FromRequestParts<S> for CurrentUserFromCookie
where
    S: Send + Sync,
    Deployment: FromRef<S>,
{
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> Result<CurrentUserFromCookie, ApiError> {
        let deployment = Deployment::from_ref(state);
        let token = cookie_token_of(parts).ok_or_else(sign_in_first)?;
        let user = users::get_by_token(&deployment, &token).await?;
        Ok(CurrentUserFromCookie(user))
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
    if let Some(token) = bearer_token_of(parts)? {
        return Ok(token);
    }
    cookie_token_of(parts).ok_or_else(sign_in_first)
}

/// The token of the browser's cookie, if the request has it.
fn cookie_token_of(parts: &Parts) -> Option<String> {
    CookieJar::from_headers(&parts.headers)
        .get(TOKEN_COOKIE)
        .map(|cookie| cookie.value().to_string())
}

/// The `Authorization: Bearer` token, if the request has the header.
fn bearer_token_of(parts: &Parts) -> Result<Option<String>, ApiError> {
    let Some(header) = parts.headers.get(AUTHORIZATION) else {
        return Ok(None);
    };
    let token = header
        .to_str()
        .ok()
        .and_then(|header| header.strip_prefix("Bearer "))
        .ok_or_else(|| ApiError::new(StatusCode::UNAUTHORIZED, "a bearer token is needed"))?;
    Ok(Some(token.trim().to_string()))
}

fn sign_in_first() -> ApiError {
    ApiError::new(StatusCode::UNAUTHORIZED, "Sign in first")
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
