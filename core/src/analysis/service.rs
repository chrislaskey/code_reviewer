use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::call_trace::{get_call_trace, CallTraceOptions, GetCallTraceResponse};
use super::error::AnalysisError;
use super::file_changes::{group_changes_by_file, FileChange};
use super::function_changes::{list_function_changes, FunctionChange};
use super::function_source::{get_function_source, FunctionSource};
use super::model::FunctionId;
use super::project_index::{CallDirection, ProjectIndex, ProjectIndexCache, SkippedFile};
use crate::git::{
    get_commit_sha, get_parent_commit_sha, list_changed_line_ranges, ListChangedLineRangesRequest,
};

/// Request to analyze what changed, function by function.
///
/// - Neither ref: uncommitted changes against `HEAD`.
/// - `base_ref` only: the working tree against `base_ref`.
/// - `head_ref` only: that commit against its parent (one commit).
/// - Both: `head_ref` against `base_ref` (a revision range).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzeChangesRequest {
    pub repo_path: String,
    pub base_ref: Option<String>,
    pub head_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzeChangesResponse {
    pub base_commit: String,
    /// `None` when the head is the working tree.
    pub head_commit: Option<String>,
    pub changes: Vec<FunctionChange>,
    /// Files in the head that could not be indexed.
    pub skipped_files: Vec<SkippedFile>,
}

/// The changes of one review, grouped by file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzeFileChangesResponse {
    pub base_commit: String,
    /// `None` when the head is the working tree.
    pub head_commit: Option<String>,
    /// One entry per changed file, sorted by path.
    pub files: Vec<FileChange>,
    /// Files in the head that could not be indexed.
    pub skipped_files: Vec<SkippedFile>,
}

/// Request for the full source of one function.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetFunctionSourceRequest {
    pub repo_path: String,
    /// Read from this revision. `None` reads the working tree.
    pub revision: Option<String>,
    pub function: FunctionId,
}

/// Request for the full call tree above or below one function.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetCallTraceRequest {
    pub repo_path: String,
    /// Trace through this revision. `None` traces the working tree.
    pub revision: Option<String>,
    pub function: FunctionId,
    pub direction: CallDirection,
    pub max_depth: Option<usize>,
    pub max_node_count: Option<usize>,
}

/// The analysis entry point shared by the Axum and Tauri apps.
///
/// Holds the index cache, so one instance should live for the whole app.
#[derive(Debug, Default)]
pub struct AnalysisService {
    cache: ProjectIndexCache,
}

impl AnalysisService {
    pub fn new() -> Self {
        Self::default()
    }

    /// List the functions added, modified, removed or renamed, each with its one-level external calls.
    pub fn analyze_changes(
        &self,
        request: AnalyzeChangesRequest,
    ) -> Result<AnalyzeChangesResponse, AnalysisError> {
        let repo = request.repo_path.as_str();
        let head_commit = request
            .head_ref
            .as_deref()
            .map(|head_ref| get_commit_sha(repo, head_ref))
            .transpose()?;
        let base_commit = match (&request.base_ref, &head_commit) {
            (Some(base_ref), _) => get_commit_sha(repo, base_ref)?,
            (None, Some(head_commit)) => get_parent_commit_sha(repo, head_commit)?,
            (None, None) => get_commit_sha(repo, "HEAD")?,
        };

        let changed_files = list_changed_line_ranges(ListChangedLineRangesRequest {
            repo_path: request.repo_path.clone(),
            base_ref: Some(base_commit.clone()),
            head_ref: head_commit.clone(),
        })?;

        let base_index = self
            .cache
            .get_commit_project_index(Path::new(repo), &base_commit)?;
        let head_index = match &head_commit {
            Some(head_commit) => self
                .cache
                .get_commit_project_index(Path::new(repo), head_commit)?,
            None => self.cache.get_current_project_index(Path::new(repo))?,
        };

        let changes = list_function_changes(&base_index, &head_index, &changed_files.files)?;
        Ok(AnalyzeChangesResponse {
            base_commit,
            head_commit,
            changes,
            skipped_files: head_index.skipped_files().to_vec(),
        })
    }

    /// Group the changed functions by file, with the callers and calls of each file.
    ///
    /// Takes the same request as `analyze_changes`.
    pub fn analyze_file_changes(
        &self,
        request: AnalyzeChangesRequest,
    ) -> Result<AnalyzeFileChangesResponse, AnalysisError> {
        let response = self.analyze_changes(request)?;
        Ok(AnalyzeFileChangesResponse {
            base_commit: response.base_commit,
            head_commit: response.head_commit,
            files: group_changes_by_file(&response.changes),
            skipped_files: response.skipped_files,
        })
    }

    /// Get the whole function, not only the changed lines.
    pub fn get_function_source(
        &self,
        request: GetFunctionSourceRequest,
    ) -> Result<FunctionSource, AnalysisError> {
        let index = self.get_index(&request.repo_path, request.revision.as_deref())?;
        get_function_source(&index, &request.function)
    }

    /// Follow every call above or below a function, through the whole project.
    pub fn get_call_trace(
        &self,
        request: GetCallTraceRequest,
    ) -> Result<GetCallTraceResponse, AnalysisError> {
        let index = self.get_index(&request.repo_path, request.revision.as_deref())?;
        get_call_trace(
            &index,
            &request.function,
            CallTraceOptions {
                direction: request.direction,
                max_depth: request.max_depth,
                max_node_count: request.max_node_count,
            },
        )
    }

    fn get_index(
        &self,
        repo_path: &str,
        revision: Option<&str>,
    ) -> Result<Arc<ProjectIndex>, AnalysisError> {
        match revision {
            Some(revision) => {
                let commit_sha = get_commit_sha(repo_path, revision)?;
                self.cache
                    .get_commit_project_index(Path::new(repo_path), &commit_sha)
            }
            None => self.cache.get_current_project_index(Path::new(repo_path)),
        }
    }
}
