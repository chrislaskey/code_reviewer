//! Running the `code_reviewer_core` analysis from a Tauri command. The core
//! reads files and runs git, so each call runs on a blocking thread.

use std::sync::Arc;

use code_reviewer_core::analysis::error::AnalysisError;
use code_reviewer_core::analysis::service::AnalysisService;

/// Runs `work` on a blocking thread with the app's analysis service.
pub(crate) async fn run_analysis<T, F>(service: &Arc<AnalysisService>, work: F) -> Result<T, String>
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
