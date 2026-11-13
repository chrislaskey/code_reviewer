//! Tracing a test through a repository's changes (`src/traces/`): the Tauri
//! commands, each one call to the backend server the app
//! is connected to.

use code_reviewer_core::analysis::service::{
    GetTestCallStackRequest, GetTestCallStackResponse, ListTestsRequest, ListTestsResponse,
};
use tauri::State;

use crate::connection::BackendConnection;

/// Every test in the head of a comparison, marking the ones it added or modified.
#[tauri::command]
pub async fn list_tests(
    connection: State<'_, BackendConnection>,
    request: ListTestsRequest,
) -> Result<ListTestsResponse, String> {
    connection
        .client()?
        .list_tests(&request)
        .await
        .map_err(|error| error.to_string())
}

/// The calls below one test, walked top to bottom, each project function
/// shown in full with its lines marked against the base.
#[tauri::command]
pub async fn get_test_call_stack(
    connection: State<'_, BackendConnection>,
    request: GetTestCallStackRequest,
) -> Result<GetTestCallStackResponse, String> {
    connection
        .client()?
        .get_test_call_stack(&request)
        .await
        .map_err(|error| error.to_string())
}
