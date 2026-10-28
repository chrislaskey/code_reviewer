//! Tracing a test through a repository's changes (`src/traces/`): the Tauri
//! commands that list the tests of a comparison and walk the calls below one.

use code_reviewer_core::analysis::service::{
    GetTestCallStackRequest, GetTestCallStackResponse, ListTestsRequest, ListTestsResponse,
};
use tauri::State;

use crate::AppState;
use crate::analysis::run_analysis;

/// Every test in the head of a comparison, marking the ones it added or modified.
#[tauri::command]
pub async fn list_tests(
    state: State<'_, AppState>,
    request: ListTestsRequest,
) -> Result<ListTestsResponse, String> {
    run_analysis(&state.analysis, move |service| service.list_tests(request)).await
}

/// The calls below one test, walked top to bottom, each project function
/// shown in full with its lines marked against the base.
#[tauri::command]
pub async fn get_test_call_stack(
    state: State<'_, AppState>,
    request: GetTestCallStackRequest,
) -> Result<GetTestCallStackResponse, String> {
    run_analysis(&state.analysis, move |service| {
        service.get_test_call_stack(request)
    })
    .await
}
