//! The memberships API: joining and leaving a community, as JSON, for the
//! request's user.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use uuid::Uuid;

use app_core::app::memberships;
use app_core::data::app_users_communities::Membership;
use app_core::deployment::Deployment;

use crate::api_error::ApiError;
use crate::user_auth::CurrentUser;

pub async fn create(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
    Path(community_id): Path<Uuid>,
) -> Result<(StatusCode, Json<Membership>), ApiError> {
    let membership = memberships::create(&deployment, &user, community_id).await?;
    Ok((StatusCode::CREATED, Json(membership)))
}

pub async fn delete(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
    Path(community_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    memberships::delete(&deployment, &user, community_id).await?;
    Ok(StatusCode::NO_CONTENT)
}
