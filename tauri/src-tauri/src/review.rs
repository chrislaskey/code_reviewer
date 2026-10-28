//! Reviewing a repository's changes (`src/reviews/`): the Tauri commands
//! around `code_reviewer_core`. Each runs on a blocking thread, since the
//! core reads files and runs git.

use code_reviewer_core::analysis::file_diff::FileDiff;
use code_reviewer_core::analysis::function_diff::FunctionDiff;
use code_reviewer_core::analysis::service::{
    AnalyzeFileChangesRequest, AnalyzeFileChangesResponse, GetFileDiffsRequest,
    GetFunctionDiffsRequest,
};
use code_reviewer_core::git::{
    self, ListBranchesAndRecentCommitsRequest, ListBranchesAndRecentCommitsResponse,
};
use tauri::State;

use crate::AppState;
use crate::analysis::run_analysis;

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
