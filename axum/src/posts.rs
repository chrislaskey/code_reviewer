//! The posts API: `app_core::app::posts` as JSON, for the request's user.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::Deserialize;
use uuid::Uuid;

use app_core::app::posts::{self, PostWithOwnerAndCommunities};
use app_core::data::app_posts::Post;
use app_core::deployment::Deployment;

use crate::api_error::ApiError;
use crate::current_user::CurrentUser;

/// What a request writes: the fields of a post the user fills in, and
/// the communities they put it in.
#[derive(Deserialize)]
pub struct PostFields {
    pub body: String,
    #[serde(default)]
    pub community_ids: Vec<Uuid>,
}

pub async fn list(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
) -> Result<Json<Vec<PostWithOwnerAndCommunities>>, ApiError> {
    Ok(Json(posts::list(&deployment, &user).await?))
}

pub async fn get(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<Json<PostWithOwnerAndCommunities>, ApiError> {
    Ok(Json(posts::get(&deployment, &user, id).await?))
}

pub async fn create(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
    Json(fields): Json<PostFields>,
) -> Result<(StatusCode, Json<Post>), ApiError> {
    let post = posts::create(&deployment, &user, &fields.body, &fields.community_ids).await?;
    Ok((StatusCode::CREATED, Json(post)))
}

pub async fn update(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<Uuid>,
    Json(fields): Json<PostFields>,
) -> Result<Json<Post>, ApiError> {
    Ok(Json(
        posts::update(&deployment, &user, id, &fields.body, &fields.community_ids).await?,
    ))
}

pub async fn delete(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    posts::delete(&deployment, &user, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
