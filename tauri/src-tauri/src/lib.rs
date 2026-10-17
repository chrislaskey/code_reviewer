use std::sync::Arc;

use code_reviewer_core::{
    self as core, GreetRequest,
    analysis::{
        call_trace::GetCallTraceResponse,
        error::AnalysisError,
        function_source::FunctionSource,
        service::{
            AnalysisService, AnalyzeChangesRequest, AnalyzeChangesResponse,
            AnalyzeFileChangesRequest, AnalyzeFileChangesResponse, GetCallTraceRequest,
            GetFunctionSourceRequest,
        },
    },
    git::{
        ChangedFile, ListBranchesAndRecentCommitsRequest, ListBranchesAndRecentCommitsResponse,
        ListChangedFilesRequest,
    },
};
use tauri::State;

#[tauri::command]
fn greet(name: &str) -> String {
    let response = core::greet(GreetRequest {
        name: name.to_string(),
    });
    response.message
}

#[tauri::command]
fn list_changed_files(repo_path: &str) -> Result<Vec<ChangedFile>, String> {
    core::git::list_changed_files(ListChangedFilesRequest {
        repo_path: repo_path.to_string(),
    })
    .map(|r| r.files)
    .map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_branches_and_recent_commits(
    request: ListBranchesAndRecentCommitsRequest,
) -> Result<ListBranchesAndRecentCommitsResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
        core::git::list_branches_and_recent_commits(request)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}

/// Runs analysis on a blocking thread, since it reads files and runs git.
async fn run_analysis<T, F>(service: &Arc<AnalysisService>, work: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(&AnalysisService) -> Result<T, AnalysisError> + Send + 'static,
{
    let service = Arc::clone(service);
    tauri::async_runtime::spawn_blocking(move || work(&service))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn analyze_changes(
    service: State<'_, Arc<AnalysisService>>,
    request: AnalyzeChangesRequest,
) -> Result<AnalyzeChangesResponse, String> {
    run_analysis(&service, move |service| service.analyze_changes(request)).await
}

#[tauri::command]
async fn analyze_file_changes(
    service: State<'_, Arc<AnalysisService>>,
    request: AnalyzeFileChangesRequest,
) -> Result<AnalyzeFileChangesResponse, String> {
    run_analysis(&service, move |service| {
        service.analyze_file_changes(request)
    })
    .await
}

#[tauri::command]
async fn get_function_source(
    service: State<'_, Arc<AnalysisService>>,
    request: GetFunctionSourceRequest,
) -> Result<FunctionSource, String> {
    run_analysis(&service, move |service| {
        service.get_function_source(request)
    })
    .await
}

#[tauri::command]
async fn get_call_trace(
    service: State<'_, Arc<AnalysisService>>,
    request: GetCallTraceRequest,
) -> Result<GetCallTraceResponse, String> {
    run_analysis(&service, move |service| service.get_call_trace(request)).await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(Arc::new(AnalysisService::new()))
        .invoke_handler(tauri::generate_handler![
            greet,
            list_changed_files,
            list_branches_and_recent_commits,
            analyze_changes,
            analyze_file_changes,
            get_function_source,
            get_call_trace
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
