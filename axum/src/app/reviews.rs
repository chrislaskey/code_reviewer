//! The reviews API, `app_core::app::reviews`: a git repository on the
//! server's machine, its branches, and what a comparison changed. It runs
//! git and reads files there, so only an app's bearer token reaches it.
//! The POST routes are reads whose requests do not fit a URL.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Query, State};

use app_core::app::analysis::file_diff::FileDiff;
use app_core::app::analysis::function_diff::FunctionDiff;
use app_core::app::analysis::service::{
    AnalyzeFileChangesRequest, AnalyzeFileChangesResponse, GetFileDiffsRequest,
    GetFunctionDiffsRequest,
};
use app_core::app::reviews;
use app_core::app::server_state::ServerState;
use app_core::services::git::{
    ListBranchesAndRecentCommitsRequest, ListBranchesAndRecentCommitsResponse,
};

use crate::api_error::ApiError;
use crate::user_auth::CurrentUserFromBearerToken;

pub async fn branches_list(
    CurrentUserFromBearerToken(_user): CurrentUserFromBearerToken,
    Query(request): Query<ListBranchesAndRecentCommitsRequest>,
) -> Result<Json<ListBranchesAndRecentCommitsResponse>, ApiError> {
    Ok(Json(reviews::branches_list(request).await?))
}

pub async fn file_changes_list(
    State(state): State<Arc<ServerState>>,
    CurrentUserFromBearerToken(_user): CurrentUserFromBearerToken,
    Json(request): Json<AnalyzeFileChangesRequest>,
) -> Result<Json<AnalyzeFileChangesResponse>, ApiError> {
    Ok(Json(
        reviews::file_changes_list(&state.analysis, request).await?,
    ))
}

pub async fn function_diffs_list(
    State(state): State<Arc<ServerState>>,
    CurrentUserFromBearerToken(_user): CurrentUserFromBearerToken,
    Json(request): Json<GetFunctionDiffsRequest>,
) -> Result<Json<Vec<FunctionDiff>>, ApiError> {
    Ok(Json(
        reviews::function_diffs_list(&state.analysis, request).await?,
    ))
}

pub async fn file_diffs_list(
    State(state): State<Arc<ServerState>>,
    CurrentUserFromBearerToken(_user): CurrentUserFromBearerToken,
    Json(request): Json<GetFileDiffsRequest>,
) -> Result<Json<Vec<FileDiff>>, ApiError> {
    Ok(Json(
        reviews::file_diffs_list(&state.analysis, request).await?,
    ))
}
