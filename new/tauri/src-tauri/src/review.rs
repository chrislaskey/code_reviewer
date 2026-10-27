//! Reviewing a repository's changes (`src/reviews/`): the Tauri commands
//! around `code_reviewer_core`. Each runs on a blocking thread, since the
//! core reads files and runs git.

use std::sync::Arc;

use code_reviewer_core::analysis::error::AnalysisError;
use code_reviewer_core::analysis::file_diff::FileDiff;
use code_reviewer_core::analysis::function_diff::FunctionDiff;
use code_reviewer_core::analysis::service::{
    AnalysisService, AnalyzeFileChangesRequest, AnalyzeFileChangesResponse, GetFileDiffsRequest,
    GetFunctionDiffsRequest,
};
use code_reviewer_core::git::{
    self, ListBranchesAndRecentCommitsRequest, ListBranchesAndRecentCommitsResponse,
};
use tauri::State;

use crate::AppState;

/// Runs `work` on a blocking thread with the app's analysis service.
async fn run_analysis<T, F>(service: &Arc<AnalysisService>, work: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(&AnalysisService) -> Result<T, AnalysisError> + Send + 'static,
{
    let service = Arc::clone(service);
    tauri::async_runtime::spawn_blocking(move || work(&service))
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())
}

/// The branches and latest commits of a repository, to choose what to review.
#[tauri::command]
pub async fn list_branches_and_recent_commits(
    request: ListBranchesAndRecentCommitsRequest,
) -> Result<ListBranchesAndRecentCommitsResponse, String> {
    tauri::async_runtime::spawn_blocking(move || git::list_branches_and_recent_commits(request))
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())
}

/// What a comparison changed, grouped by file and by group.
#[tauri::command]
pub async fn analyze_file_changes(
    state: State<'_, AppState>,
    request: AnalyzeFileChangesRequest,
) -> Result<AnalyzeFileChangesResponse, String> {
    run_analysis(&state.analysis, move |service| {
        service.analyze_file_changes(request)
    })
    .await
}

/// The whole of each changed function, with its added and removed lines marked.
#[tauri::command]
pub async fn get_function_diffs(
    state: State<'_, AppState>,
    request: GetFunctionDiffsRequest,
) -> Result<Vec<FunctionDiff>, String> {
    run_analysis(&state.analysis, move |service| {
        service.get_function_diffs(request)
    })
    .await
}

/// The changed lines of files of any type, with a few lines of context.
#[tauri::command]
pub async fn get_file_diffs(
    state: State<'_, AppState>,
    request: GetFileDiffsRequest,
) -> Result<Vec<FileDiff>, String> {
    run_analysis(&state.analysis, move |service| {
        service.get_file_diffs(request)
    })
    .await
}
