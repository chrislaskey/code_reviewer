//! Reviews of a git repository on the server's machine: its branches and
//! recent commits, to choose what to compare, then what a comparison
//! changed, by file and group, and the diffs of its functions and files.
//! The work is `app::analysis`, run on a blocking thread, as it reads
//! files and runs git; it needs the `host` feature, on the server alone.

#[cfg(feature = "host")]
pub use on_host::*;

#[cfg(feature = "host")]
mod on_host {
    use std::sync::Arc;

    use thiserror::Error;

    use crate::app::analysis::error::AnalysisError;
    use crate::app::analysis::file_diff::FileDiff;
    use crate::app::analysis::function_diff::FunctionDiff;
    use crate::app::analysis::service::{
        AnalysisService, AnalyzeFileChangesRequest, AnalyzeFileChangesResponse,
        GetFileDiffsRequest, GetFunctionDiffsRequest,
    };
    use crate::services::git::{
        self, GitError, ListBranchesAndRecentCommitsRequest, ListBranchesAndRecentCommitsResponse,
    };

    #[derive(Debug, Error)]
    pub enum ReviewError {
        #[error(transparent)]
        Analysis(#[from] AnalysisError),
        #[error(transparent)]
        Git(#[from] GitError),
        #[error("the review stopped before it finished: {0}")]
        Interrupted(String),
    }

    /// The branches and latest commits of a repository, to choose what to review.
    pub async fn branches_list(
        request: ListBranchesAndRecentCommitsRequest,
    ) -> Result<ListBranchesAndRecentCommitsResponse, ReviewError> {
        tokio::task::spawn_blocking(move || git::list_branches_and_recent_commits(request))
            .await
            .map_err(|error| ReviewError::Interrupted(error.to_string()))?
            .map_err(ReviewError::from)
    }

    /// What a comparison changed, grouped by file and by group.
    pub async fn file_changes_list(
        analysis: &Arc<AnalysisService>,
        request: AnalyzeFileChangesRequest,
    ) -> Result<AnalyzeFileChangesResponse, ReviewError> {
        run(analysis, move |service| {
            service.analyze_file_changes(request)
        })
        .await
    }

    /// The whole of each changed function, with its added and removed lines marked.
    pub async fn function_diffs_list(
        analysis: &Arc<AnalysisService>,
        request: GetFunctionDiffsRequest,
    ) -> Result<Vec<FunctionDiff>, ReviewError> {
        run(analysis, move |service| service.get_function_diffs(request)).await
    }

    /// The changed lines of files of any type, with a few lines of context.
    pub async fn file_diffs_list(
        analysis: &Arc<AnalysisService>,
        request: GetFileDiffsRequest,
    ) -> Result<Vec<FileDiff>, ReviewError> {
        run(analysis, move |service| service.get_file_diffs(request)).await
    }

    async fn run<T, F>(analysis: &Arc<AnalysisService>, work: F) -> Result<T, ReviewError>
    where
        T: Send + 'static,
        F: FnOnce(&AnalysisService) -> Result<T, AnalysisError> + Send + 'static,
    {
        let analysis = Arc::clone(analysis);
        tokio::task::spawn_blocking(move || work(&analysis))
            .await
            .map_err(|error| ReviewError::Interrupted(error.to_string()))?
            .map_err(ReviewError::from)
    }
}
