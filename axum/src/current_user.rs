//! Who a request is from: the user its token stands for, see
//! `app_core::app::users::get_by_token`. The apps send the token as
//! `Authorization: Bearer`; a browser sends the `token` cookie a sign-in
//! set, see `accounts`. A request with no token, or one the server does
//! not know, is answered 401, whatever it asks for.

use axum::extract::{FromRef, FromRequestParts, State};
use axum::http::StatusCode;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use axum_extra::extract::CookieJar;
use serde::{Deserialize, Serialize};

use app_core::accounts::plans::{self, Plan};
use app_core::accounts::users as accounts_users;
use app_core::app::users::{self, Kind, User};
use app_core::deployment::Deployment;
use app_core::ops::staff::{self, StaffError};
use chrono::{DateTime, Utc};
use uuid::Uuid;

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

/// The request's user as their own pages show them: with their
/// account's email, which nobody else is sent, whether they are staff,
/// for the link to the ops app, whether they are a trial user, with
/// when the trial ends, for the banner, and their plan, with the
/// subscription whose payment failed, for that banner.
#[derive(Serialize)]
pub struct CurrentUserShown {
    #[serde(flatten)]
    pub user: User,
    pub email: String,
    pub is_staff: bool,
    pub kind: Kind,
    pub trial_expires_at: Option<DateTime<Utc>>,
    pub plan: Option<Plan>,
    pub payment_failed_subscription_id: Option<Uuid>,
}

/// `GET /api/current-user`: the user the request is from.
pub async fn get(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
) -> Result<axum::Json<CurrentUserShown>, ApiError> {
    shown(&deployment, user).await.map(axum::Json)
}

#[derive(Deserialize)]
pub struct CurrentUserUpdate {
    pub name: String,
}

/// `PUT /api/current-user`: the user's new name.
pub async fn update(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
    axum::Json(update): axum::Json<CurrentUserUpdate>,
) -> Result<axum::Json<CurrentUserShown>, ApiError> {
    let user = users::update(&deployment, &user, &update.name).await?;
    shown(&deployment, user).await.map(axum::Json)
}

async fn shown(deployment: &Deployment, user: User) -> Result<CurrentUserShown, ApiError> {
    let account = accounts_users::get(deployment, user.id).await?;
    let is_staff = match staff::get(deployment, &user).await {
        Ok(_) => true,
        Err(StaffError::NotStaff) => false,
        Err(error) => return Err(error.into()),
    };
    let kind = match account.trial_expires_at {
        Some(_) => Kind::Trial,
        None => Kind::Full,
    };
    let plan = plans::current(deployment, &user).await?;
    let payment_failed_subscription_id = plans::payment_failed(deployment, &user).await?;
    Ok(CurrentUserShown {
        user,
        email: account.email,
        is_staff,
        kind,
        trial_expires_at: account.trial_expires_at,
        plan,
        payment_failed_subscription_id,
    })
}
