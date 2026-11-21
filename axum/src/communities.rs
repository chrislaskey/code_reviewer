//! The communities API: `app_core::app::communities` as JSON, for the
//! request's user.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::Deserialize;
use uuid::Uuid;

use app_core::app::communities::{self, Community, CommunityAndMembership};
use app_core::deployment::Deployment;

use crate::api_error::ApiError;
use crate::current_user::CurrentUser;

/// What a request writes: the fields of a community the user fills in.
#[derive(Deserialize)]
pub struct CommunityFields {
    pub name: String,
}

pub async fn list(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
) -> Result<Json<Vec<CommunityAndMembership>>, ApiError> {
    Ok(Json(communities::list(&deployment, &user).await?))
}

pub async fn get(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<Json<CommunityAndMembership>, ApiError> {
    Ok(Json(communities::get(&deployment, &user, id).await?))
}

pub async fn create(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
    Json(fields): Json<CommunityFields>,
) -> Result<(StatusCode, Json<Community>), ApiError> {
    let community = communities::create(&deployment, &user, &fields.name).await?;
    Ok((StatusCode::CREATED, Json(community)))
}

pub async fn update(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<Uuid>,
    Json(fields): Json<CommunityFields>,
) -> Result<Json<Community>, ApiError> {
    Ok(Json(
        communities::update(&deployment, &user, id, &fields.name).await?,
    ))
}

pub async fn delete(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    communities::delete(&deployment, &user, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
