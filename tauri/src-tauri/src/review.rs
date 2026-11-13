//! Reviewing a repository's changes (`src/reviews/`): the Tauri commands,
//! each one call to the backend server the app
//! is connected to.

use code_reviewer_core::analysis::file_diff::FileDiff;
use code_reviewer_core::analysis::function_diff::FunctionDiff;
use code_reviewer_core::analysis::service::{
    AnalyzeFileChangesRequest, AnalyzeFileChangesResponse, GetFileDiffsRequest,
    GetFunctionDiffsRequest,
};
use code_reviewer_core::git::{
    ListBranchesAndRecentCommitsRequest, ListBranchesAndRecentCommitsResponse,
};
use tauri::State;

use crate::connection::BackendConnection;

/// The branches and latest commits of a repository, to choose what to review.
#[tauri::command]
pub async fn list_branches_and_recent_commits(
    connection: State<'_, BackendConnection>,
    request: ListBranchesAndRecentCommitsRequest,
) -> Result<ListBranchesAndRecentCommitsResponse, String> {
    connection
        .client()?
        .list_branches_and_recent_commits(&request)
        .await
        .map_err(|error| error.to_string())
}

/// What a comparison changed, grouped by file and by group.
#[tauri::command]
pub async fn analyze_file_changes(
    connection: State<'_, BackendConnection>,
    request: AnalyzeFileChangesRequest,
) -> Result<AnalyzeFileChangesResponse, String> {
    connection
        .client()?
        .analyze_file_changes(&request)
        .await
        .map_err(|error| error.to_string())
}

/// The whole of each changed function, with its added and removed lines marked.
#[tauri::command]
pub async fn get_function_diffs(
    connection: State<'_, BackendConnection>,
    request: GetFunctionDiffsRequest,
) -> Result<Vec<FunctionDiff>, String> {
    connection
        .client()?
        .get_function_diffs(&request)
        .await
        .map_err(|error| error.to_string())
}

/// The changed lines of files of any type, with a few lines of context.
#[tauri::command]
pub async fn get_file_diffs(
    connection: State<'_, BackendConnection>,
    request: GetFileDiffsRequest,
) -> Result<Vec<FileDiff>, String> {
    connection
        .client()?
        .get_file_diffs(&request)
        .await
        .map_err(|error| error.to_string())
}
