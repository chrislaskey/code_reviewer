use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::call_trace::{get_call_trace, CallTraceOptions, GetCallTraceResponse};
use super::error::AnalysisError;
use super::file_changes::{add_unparsed_files, group_changes_by_file, FileChange};
use super::file_diff::{get_file_diff, FileDiff, FileDiffTarget};
use super::function_changes::{list_function_changes, FunctionChange};
use super::function_diff::{get_function_diff, FunctionDiff, FunctionDiffTarget};
use super::function_source::{get_function_source, FunctionSource};
use super::group_changes::{group_changed_files, GroupRow};
use super::model::FunctionId;
use super::module_rules::ModuleRules;
use super::project_index::{CallDirection, ProjectIndex, ProjectIndexCache, SkippedFile};
use crate::git::{
    get_commit_sha, get_merge_base_commit_sha, get_parent_commit_sha, list_changed_line_ranges,
    list_changed_paths, ListChangedLineRangesRequest,
};
use crate::groups::{load_group_config, GroupFlags, GroupMatcher, GroupSummary};

/// Request to analyze what changed, function by function.
///
/// - Neither ref: uncommitted changes against `HEAD`.
/// - `base_ref` only: the working tree against `base_ref`.
/// - `head_ref` only: that commit against its parent (one commit).
/// - Both: `head_ref` against `base_ref` (a revision range).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AnalyzeChangesRequest {
    pub repo_path: String,
    pub base_ref: Option<String>,
    pub head_ref: Option<String>,
    /// With both refs, compare from where `head_ref` branched off `base_ref`,
    /// like a pull request, instead of against the tip of `base_ref`.
    #[serde(default)]
    pub compare_from_merge_base: bool,
}

/// Request for the changes of one review, grouped by file.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AnalyzeFileChangesRequest {
    #[serde(flatten)]
    pub changes: AnalyzeChangesRequest,
    /// Which callers and calls to highlight or leave out.
    #[serde(default)]
    pub module_rules: ModuleRules,
    /// Which groups to place the changed files in. With none defined (no
    /// `.code_reviewer.toml` and no flags), every file is in the default group.
    #[serde(default)]
    pub group_flags: GroupFlags,
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
    /// One entry per changed file of any type, sorted by path.
    pub files: Vec<FileChange>,
    /// Files in the head that could not be indexed.
    pub skipped_files: Vec<SkippedFile>,
    /// The groups that hold changed files, in config order.
    pub groups: Vec<GroupSummary>,
    /// One row for each group in `groups`.
    pub group_rows: Vec<GroupRow>,
    /// A row that rolls up every group.
    pub all_changes_row: GroupRow,
}

/// Request for the full source of one function.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetFunctionSourceRequest {
    pub repo_path: String,
    /// Read from this revision. `None` reads the working tree.
    pub revision: Option<String>,
    pub function: FunctionId,
}

/// Request for the whole source of changed functions, with the changed lines marked.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetFunctionDiffsRequest {
    pub repo_path: String,
    /// The base commit of the comparison (`base_commit` of the file changes).
    pub base_commit: String,
    /// The head commit of the comparison. `None` is the working tree.
    pub head_commit: Option<String>,
    pub targets: Vec<FunctionDiffTarget>,
}

/// Request for the changed lines of files, with a few lines of context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetFileDiffsRequest {
    pub repo_path: String,
    /// The base commit of the comparison (`base_commit` of the file changes).
    pub base_commit: String,
    /// The head commit of the comparison. `None` is the working tree.
    pub head_commit: Option<String>,
    pub targets: Vec<FileDiffTarget>,
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

/// A finished comparison, with the two indexes it was made from.
struct ChangesAnalysis {
    response: AnalyzeChangesResponse,
    base_index: Arc<ProjectIndex>,
    head_index: Arc<ProjectIndex>,
}

/// Look up the file that defines each called project function, in the head or else the base.
fn set_callee_file_paths(
    files: &mut [FileChange],
    head_index: &ProjectIndex,
    base_index: &ProjectIndex,
) {
    for callee in files.iter_mut().flat_map(|file| file.callees.iter_mut()) {
        if !callee.defined_in_project {
            continue;
        }
        callee.file_path = head_index
            .get_function(&callee.function)
            .or_else(|| base_index.get_function(&callee.function))
            .map(|definition| definition.file_path.clone());
    }
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
        self.analyze_changes_with_indexes(request)
            .map(|analysis| analysis.response)
    }

    fn analyze_changes_with_indexes(
        &self,
        request: AnalyzeChangesRequest,
    ) -> Result<ChangesAnalysis, AnalysisError> {
        let repo = request.repo_path.as_str();
        let head_commit = request
            .head_ref
            .as_deref()
            .map(|head_ref| get_commit_sha(repo, head_ref))
            .transpose()?;
        let base_commit = match (&request.base_ref, &head_commit) {
            (Some(base_ref), Some(head_commit)) if request.compare_from_merge_base => {
                get_merge_base_commit_sha(repo, base_ref, head_commit)?
            }
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
        let response = AnalyzeChangesResponse {
            base_commit,
            head_commit,
            changes,
            skipped_files: head_index.skipped_files().to_vec(),
        };
        Ok(ChangesAnalysis {
            response,
            base_index,
            head_index,
        })
    }

    /// Group the changed functions by file, with the callers and calls of each file.
    ///
    /// Also places the files in groups when any are defined, and rolls them up into group rows.
    pub fn analyze_file_changes(
        &self,
        request: AnalyzeFileChangesRequest,
    ) -> Result<AnalyzeFileChangesResponse, AnalysisError> {
        let group_config = load_group_config(&request.changes.repo_path, &request.group_flags)?;
        let matcher = GroupMatcher::compile(&group_config)?;

        let repo_path = request.changes.repo_path.clone();
        let analysis = self.analyze_changes_with_indexes(request.changes)?;
        let mut files = group_changes_by_file(&analysis.response.changes, &request.module_rules);
        let changed_paths = list_changed_paths(
            &repo_path,
            &analysis.response.base_commit,
            analysis.response.head_commit.as_deref(),
        )?;
        add_unparsed_files(&mut files, &changed_paths);
        set_callee_file_paths(&mut files, &analysis.head_index, &analysis.base_index);
        let grouped = group_changed_files(&mut files, &matcher);

        Ok(AnalyzeFileChangesResponse {
            base_commit: analysis.response.base_commit,
            head_commit: analysis.response.head_commit,
            files,
            skipped_files: analysis.response.skipped_files,
            groups: grouped.groups,
            group_rows: grouped.rows,
            all_changes_row: grouped.all_changes_row,
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

    /// Get the whole of each changed function, with its added and removed lines marked.
    pub fn get_function_diffs(
        &self,
        request: GetFunctionDiffsRequest,
    ) -> Result<Vec<FunctionDiff>, AnalysisError> {
        let base_index = self.get_index(&request.repo_path, Some(&request.base_commit))?;
        let head_index = self.get_index(&request.repo_path, request.head_commit.as_deref())?;
        request
            .targets
            .iter()
            .map(|target| get_function_diff(&base_index, &head_index, target))
            .collect()
    }

    /// Get the changed lines of each file, with a few lines of context around them.
    pub fn get_file_diffs(
        &self,
        request: GetFileDiffsRequest,
    ) -> Result<Vec<FileDiff>, AnalysisError> {
        request
            .targets
            .iter()
            .map(|target| {
                get_file_diff(
                    &request.repo_path,
                    &request.base_commit,
                    request.head_commit.as_deref(),
                    target,
                )
            })
            .collect()
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
