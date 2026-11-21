//! The memberships API: joining and leaving a community, as JSON, for the
//! request's user.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use uuid::Uuid;

use app_core::app::memberships::{self, Membership};
use app_core::deployment::Deployment;

use crate::api_error::ApiError;
use crate::current_user::CurrentUser;

pub async fn join(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
    Path(community_id): Path<Uuid>,
) -> Result<Json<Membership>, ApiError> {
    Ok(Json(
        memberships::join(&deployment, &user, community_id).await?,
    ))
}

pub async fn leave(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
    Path(community_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    memberships::leave(&deployment, &user, community_id).await?;
    Ok(StatusCode::NO_CONTENT)
}
