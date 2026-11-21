//! The current community API: the community the browser works in, kept
//! in a cookie of its own, see `app_core::app::current_community`. A read
//! stores the community it answers, so a first read, or a choice that
//! no longer holds, settles on the user's first community.

use axum::Json;
use axum::extract::State;
use axum::http::HeaderMap;
use axum_extra::extract::CookieJar;
use axum_extra::extract::cookie::{Cookie, SameSite};
use serde::Deserialize;
use time::Duration;
use uuid::Uuid;

use app_core::app::current_community;
use app_core::data::app_communities::Community;
use app_core::deployment::Deployment;

use crate::accounts::is_localhost;
use crate::api_error::ApiError;
use crate::current_user::CurrentUser;

/// The cookie a browser keeps the current community's id in.
pub const CURRENT_COMMUNITY_COOKIE: &str = "current_community_id";

#[derive(Deserialize)]
pub struct CurrentCommunityFields {
    pub community_id: Uuid,
}

pub async fn get(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
    headers: HeaderMap,
    jar: CookieJar,
) -> Result<(CookieJar, Json<Option<Community>>), ApiError> {
    let chosen_id = jar
        .get(CURRENT_COMMUNITY_COOKIE)
        .and_then(|cookie| Uuid::parse_str(cookie.value()).ok());
    let community = current_community::get(&deployment, &user, chosen_id).await?;
    let jar = match &community {
        Some(community) if Some(community.id) != chosen_id => {
            jar.add(current_community_cookie(community.id, &headers))
        }
        Some(_) => jar,
        None => jar.remove(Cookie::build(CURRENT_COMMUNITY_COOKIE).path("/")),
    };
    Ok((jar, Json(community)))
}

pub async fn update(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
    headers: HeaderMap,
    jar: CookieJar,
    Json(fields): Json<CurrentCommunityFields>,
) -> Result<(CookieJar, Json<Community>), ApiError> {
    let community = current_community::update(&deployment, &user, fields.community_id).await?;
    let jar = jar.add(current_community_cookie(community.id, &headers));
    Ok((jar, Json(community)))
}

/// Kept for a year, so the choice outlasts the browser's session; `Secure`
/// unless the request came to localhost.
fn current_community_cookie(id: Uuid, headers: &HeaderMap) -> Cookie<'static> {
    Cookie::build((CURRENT_COMMUNITY_COOKIE, id.to_string()))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Strict)
        .secure(!is_localhost(headers))
        .max_age(Duration::days(365))
        .build()
}
