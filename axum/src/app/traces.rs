//! The traces API, `app_core::app::traces`: the tests of a git repository
//! on the server's machine and the calls below one of them. It reads
//! files there, so only an app's bearer token reaches it. Both routes are
//! reads whose requests do not fit a URL.

use std::sync::Arc;

use axum::Json;
use axum::extract::State;

use app_core::app::analysis::service::{
    GetTestCallStackRequest, GetTestCallStackResponse, ListTestsRequest, ListTestsResponse,
};
use app_core::app::server_state::ServerState;
use app_core::app::traces;

use crate::api_error::ApiError;
use crate::user_auth::CurrentUserFromBearerToken;

pub async fn tests_list(
    State(state): State<Arc<ServerState>>,
    CurrentUserFromBearerToken(_user): CurrentUserFromBearerToken,
    Json(request): Json<ListTestsRequest>,
) -> Result<Json<ListTestsResponse>, ApiError> {
    Ok(Json(traces::tests_list(&state.analysis, request).await?))
}

pub async fn call_stacks_get(
    State(state): State<Arc<ServerState>>,
    CurrentUserFromBearerToken(_user): CurrentUserFromBearerToken,
    Json(request): Json<GetTestCallStackRequest>,
) -> Result<Json<GetTestCallStackResponse>, ApiError> {
    Ok(Json(
        traces::call_stacks_get(&state.analysis, request).await?,
    ))
}
