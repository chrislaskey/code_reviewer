use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use super::error::AnalysisError;
use super::language::find_analyzer_for_path;
use super::model::{FileSymbols, FunctionDefinition, FunctionId, RawCallTarget};
use crate::git::{
    get_file_text_at_commit, list_file_paths_at_commit, list_file_texts_at_commit,
    list_project_file_paths, ListProjectFilePathsRequest,
};

/// A source file held in memory, used to index without touching disk.
#[derive(Debug, Clone)]
pub struct SourceFile {
    pub path: String,
    pub text: String,
}

/// Where the files of an index came from, and so where to read their text later.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum SourceOrigin {
    /// Files on disk under the repository path.
    WorkingTree,
    /// Files as committed, identified by full commit SHA.
    Commit(String),
    /// Built from text passed in. File text cannot be read again.
    #[default]
    InMemory,
}

/// A file that could not be indexed, with the reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkippedFile {
    pub path: String,
    pub reason: String,
}

/// One call from `caller` to `callee` at a line of `file_path`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallEdge {
    pub caller: FunctionId,
    pub callee: FunctionId,
    pub file_path: String,
    pub line: usize,
    pub callee_defined_in_project: bool,
}

/// Which side of a function to follow calls from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CallDirection {
    /// Functions that call the given function.
    IncomingCalls,
    /// Functions the given function calls.
    OutgoingCalls,
}

/// All calls between one function and one other function, grouped by line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallLink {
    /// The function at the other end of the call.
    pub function: FunctionId,
    /// False for functions the project does not define (standard library, deps).
    pub defined_in_project: bool,
    /// File containing the calling code.
    pub call_file_path: String,
    pub call_lines: Vec<usize>,
    /// True when caller and callee are in different modules.
    pub crosses_module_boundary: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileFingerprint {
    modified: Option<SystemTime>,
    len: u64,
}

#[derive(Debug, Clone)]
struct IndexedFile {
    symbols: Arc<FileSymbols>,
    fingerprint: Option<FileFingerprint>,
}

/// Every function and resolved call in a project, built from tree-sitter parses.
///
/// Immutable once built, so it can be shared behind an `Arc` and queried from
/// many requests at once.
#[derive(Debug, Default)]
pub struct ProjectIndex {
    repo_path: PathBuf,
    origin: SourceOrigin,
    files: BTreeMap<String, IndexedFile>,
    functions: BTreeMap<FunctionId, FunctionDefinition>,
    functions_by_module_and_name: HashMap<(String, String), Vec<FunctionId>>,
    outgoing_calls: HashMap<FunctionId, Vec<CallEdge>>,
    incoming_calls: HashMap<FunctionId, Vec<CallEdge>>,
    skipped_files: Vec<SkippedFile>,
}

impl ProjectIndex {
    pub fn repo_path(&self) -> &Path {
        &self.repo_path
    }

    pub fn origin(&self) -> &SourceOrigin {
        &self.origin
    }

    /// Read the text of an indexed file from the snapshot this index was built from.
    pub fn read_file_text(&self, file_path: &str) -> Result<String, AnalysisError> {
        let failed = |message: String| AnalysisError::ReadFileFailed {
            path: file_path.to_string(),
            message,
        };
        match &self.origin {
            SourceOrigin::WorkingTree => std::fs::read_to_string(self.repo_path.join(file_path))
                .map_err(|error| failed(error.to_string())),
            SourceOrigin::Commit(commit_sha) => {
                get_file_text_at_commit(&self.repo_path.to_string_lossy(), commit_sha, file_path)
                    .map_err(|error| failed(error.to_string()))
            }
            SourceOrigin::InMemory => {
                Err(failed("index was built from in-memory text".to_string()))
            }
        }
    }

    pub fn skipped_files(&self) -> &[SkippedFile] {
        &self.skipped_files
    }

    pub fn function_count(&self) -> usize {
        self.functions.len()
    }

    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    pub fn get_function(&self, id: &FunctionId) -> Option<&FunctionDefinition> {
        self.functions.get(id)
    }

    /// Every function in the project, ordered by module, name and arity.
    pub fn list_functions(&self) -> impl Iterator<Item = &FunctionDefinition> {
        self.functions.values()
    }

    /// Functions defined in `file_path`, in source order.
    pub fn list_functions_in_file(&self, file_path: &str) -> &[FunctionDefinition] {
        self.files
            .get(file_path)
            .map_or(&[], |file| file.symbols.functions.as_slice())
    }

    /// Calls made by other functions into `id`, grouped per caller and file.
    pub fn list_incoming_call_links(&self, id: &FunctionId) -> Vec<CallLink> {
        let edges = self.incoming_calls.get(id).map_or(&[][..], Vec::as_slice);
        group_edges(edges, id, CallDirection::IncomingCalls)
    }

    /// Calls `id` makes to other functions, grouped per callee.
    pub fn list_outgoing_call_links(&self, id: &FunctionId) -> Vec<CallLink> {
        let edges = self.outgoing_calls.get(id).map_or(&[][..], Vec::as_slice);
        group_edges(edges, id, CallDirection::OutgoingCalls)
    }

    pub fn list_call_links(&self, id: &FunctionId, direction: CallDirection) -> Vec<CallLink> {
        match direction {
            CallDirection::IncomingCalls => self.list_incoming_call_links(id),
            CallDirection::OutgoingCalls => self.list_outgoing_call_links(id),
        }
    }

    fn from_indexed_files(
        repo_path: PathBuf,
        origin: SourceOrigin,
        files: BTreeMap<String, IndexedFile>,
        skipped_files: Vec<SkippedFile>,
    ) -> Self {
        let mut index = ProjectIndex {
            repo_path,
            origin,
            files,
            skipped_files,
            ..Default::default()
        };
        index.collect_functions();
        index.resolve_calls();
        index
    }

    fn collect_functions(&mut self) {
        for file in self.files.values() {
            for function in &file.symbols.functions {
                self.functions_by_module_and_name
                    .entry((function.id.module.clone(), function.id.name.clone()))
                    .or_default()
                    .push(function.id.clone());
                self.functions.insert(function.id.clone(), function.clone());
            }
        }
    }

    fn resolve_calls(&mut self) {
        let mut edges = Vec::new();
        for file in self.files.values() {
            for call in &file.symbols.calls {
                if let Some((callee, defined)) = self.resolve_call_target(&call.target) {
                    edges.push(CallEdge {
                        caller: call.caller.clone(),
                        callee,
                        file_path: file.symbols.file_path.clone(),
                        line: call.line,
                        callee_defined_in_project: defined,
                    });
                }
            }
        }

        for edge in edges {
            self.incoming_calls
                .entry(edge.callee.clone())
                .or_default()
                .push(edge.clone());
            self.outgoing_calls
                .entry(edge.caller.clone())
                .or_default()
                .push(edge);
        }
    }

    /// Returns the callee and whether the project defines it. Local calls
    /// that match nothing (`if`, `Kernel` functions, `use` injections) are dropped.
    fn resolve_call_target(&self, target: &RawCallTarget) -> Option<(FunctionId, bool)> {
        match target {
            RawCallTarget::Remote {
                module,
                name,
                arity,
            } => Some(match self.find_defined_function(module, name, *arity) {
                Some(id) => (id, true),
                None => (FunctionId::new(module, name, *arity), false),
            }),
            RawCallTarget::Local {
                module,
                name,
                arity,
                imports,
            } => {
                if let Some(id) = self.find_defined_function(module, name, *arity) {
                    return Some((id, true));
                }
                imports
                    .iter()
                    .filter(|directive| directive.brings_into_scope(name, *arity))
                    .find_map(|directive| {
                        self.find_defined_function(&directive.module, name, *arity)
                    })
                    .map(|id| (id, true))
            }
        }
    }

    fn find_defined_function(&self, module: &str, name: &str, arity: usize) -> Option<FunctionId> {
        self.functions_by_module_and_name
            .get(&(module.to_string(), name.to_string()))?
            .iter()
            .find(|id| {
                self.functions
                    .get(id)
                    .is_some_and(|function| function.accepts_arity(arity))
            })
            .cloned()
    }
}

/// Merges edges that connect the same two functions from the same file.
fn group_edges(edges: &[CallEdge], id: &FunctionId, direction: CallDirection) -> Vec<CallLink> {
    let mut grouped: BTreeMap<(FunctionId, String), CallLink> = BTreeMap::new();
    for edge in edges {
        let other = match direction {
            CallDirection::IncomingCalls => &edge.caller,
            CallDirection::OutgoingCalls => &edge.callee,
        };
        let defined_in_project = match direction {
            CallDirection::IncomingCalls => true,
            CallDirection::OutgoingCalls => edge.callee_defined_in_project,
        };
        let link = grouped
            .entry((other.clone(), edge.file_path.clone()))
            .or_insert_with(|| CallLink {
                function: other.clone(),
                defined_in_project,
                call_file_path: edge.file_path.clone(),
                call_lines: Vec::new(),
                crosses_module_boundary: other.module != id.module,
            });
        link.call_lines.push(edge.line);
    }

    let mut links: Vec<CallLink> = grouped.into_values().collect();
    for link in &mut links {
        link.call_lines.sort_unstable();
        link.call_lines.dedup();
    }
    links
}

fn parse_source(file: &SourceFile) -> Result<Arc<FileSymbols>, SkippedFile> {
    let skipped = |reason: String| SkippedFile {
        path: file.path.clone(),
        reason,
    };
    let analyzer = find_analyzer_for_path(&file.path)
        .ok_or_else(|| skipped("no analyzer for this file type".to_string()))?;
    analyzer
        .parse_file(&file.path, &file.text)
        .map(Arc::new)
        .map_err(|error| skipped(error.to_string()))
}

/// Builds an index from in-memory sources. Files with unknown extensions are skipped.
pub fn build_project_index_from_sources(sources: Vec<SourceFile>) -> ProjectIndex {
    index_sources(PathBuf::new(), SourceOrigin::InMemory, sources)
}

fn index_sources(
    repo_path: PathBuf,
    origin: SourceOrigin,
    sources: Vec<SourceFile>,
) -> ProjectIndex {
    let results: Vec<_> = sources
        .par_iter()
        .map(|file| (file, parse_source(file)))
        .collect();

    let mut files = BTreeMap::new();
    let mut skipped_files = Vec::new();
    for (file, result) in results {
        match result {
            Ok(symbols) => {
                files.insert(
                    file.path.clone(),
                    IndexedFile {
                        symbols,
                        fingerprint: None,
                    },
                );
            }
            Err(skipped) => skipped_files.push(skipped),
        }
    }
    ProjectIndex::from_indexed_files(repo_path, origin, files, skipped_files)
}

/// Builds an index of every analyzable file as it was in `commit_sha`.
pub fn build_project_index_at_commit(
    repo_path: &Path,
    commit_sha: &str,
) -> Result<ProjectIndex, AnalysisError> {
    let repo = repo_path.to_string_lossy();
    let analyzable_paths: Vec<String> = list_file_paths_at_commit(&repo, commit_sha)?
        .into_iter()
        .filter(|path| find_analyzer_for_path(path).is_some())
        .collect();

    let sources = list_file_texts_at_commit(&repo, commit_sha, &analyzable_paths)?
        .into_iter()
        .map(|(path, text)| SourceFile { path, text })
        .collect();
    Ok(index_sources(
        repo_path.to_path_buf(),
        SourceOrigin::Commit(commit_sha.to_string()),
        sources,
    ))
}

/// Builds an index of the project at `repo_path`.
///
/// With `previous`, files whose modified time and size are unchanged reuse
/// their earlier parse, and the whole previous index is returned when no file
/// changed at all.
pub fn build_project_index(
    repo_path: &Path,
    previous: Option<&Arc<ProjectIndex>>,
) -> Result<Arc<ProjectIndex>, AnalysisError> {
    let listing = list_project_file_paths(ListProjectFilePathsRequest {
        repo_path: repo_path.to_string_lossy().to_string(),
    })?;

    let analyzable_paths: Vec<String> = listing
        .file_paths
        .into_iter()
        .filter(|path| find_analyzer_for_path(path).is_some())
        .collect();

    let results: Vec<(String, Result<IndexedFile, SkippedFile>, bool)> = analyzable_paths
        .par_iter()
        .map(|path| {
            let (result, reused) = index_file_from_disk(repo_path, path, previous);
            (path.clone(), result, reused)
        })
        .collect();

    let nothing_changed = previous.is_some_and(|previous| {
        results.iter().all(|(_, _, reused)| *reused) && results.len() == previous.files.len()
    });
    if nothing_changed {
        if let Some(previous) = previous {
            return Ok(Arc::clone(previous));
        }
    }

    let mut files = BTreeMap::new();
    let mut skipped_files = Vec::new();
    for (path, result, _) in results {
        match result {
            Ok(file) => {
                files.insert(path, file);
            }
            Err(skipped) => skipped_files.push(skipped),
        }
    }
    Ok(Arc::new(ProjectIndex::from_indexed_files(
        repo_path.to_path_buf(),
        SourceOrigin::WorkingTree,
        files,
        skipped_files,
    )))
}

/// Returns the indexed file and whether it was reused from `previous`.
fn index_file_from_disk(
    repo_path: &Path,
    path: &str,
    previous: Option<&Arc<ProjectIndex>>,
) -> (Result<IndexedFile, SkippedFile>, bool) {
    let skipped = |reason: String| SkippedFile {
        path: path.to_string(),
        reason,
    };
    let full_path = repo_path.join(path);

    let fingerprint = match std::fs::metadata(&full_path) {
        Ok(metadata) => FileFingerprint {
            modified: metadata.modified().ok(),
            len: metadata.len(),
        },
        // Listed by git but gone from disk, e.g. deleted and not yet staged.
        Err(error) => return (Err(skipped(error.to_string())), false),
    };

    let earlier = previous.and_then(|previous| previous.files.get(path));
    if let Some(earlier) = earlier.filter(|file| file.fingerprint.as_ref() == Some(&fingerprint)) {
        return (Ok(earlier.clone()), true);
    }

    let text = match std::fs::read_to_string(&full_path) {
        Ok(text) => text,
        Err(error) => return (Err(skipped(error.to_string())), false),
    };
    let result = parse_source(&SourceFile {
        path: path.to_string(),
        text,
    })
    .map(|symbols| IndexedFile {
        symbols,
        fingerprint: Some(fingerprint),
    });
    (result, false)
}

/// How many commit indexes the cache keeps before dropping the oldest.
const MAX_COMMIT_INDEXES: usize = 8;

#[derive(Debug, Default)]
struct CachedIndexes {
    working_trees: HashMap<PathBuf, Arc<ProjectIndex>>,
    commits: HashMap<(PathBuf, String), Arc<ProjectIndex>>,
    commit_insertion_order: VecDeque<(PathBuf, String)>,
}

/// Keeps indexes so later requests skip re-parsing.
///
/// A working tree index is re-checked on each lookup: file modified times
/// and sizes are compared and only the changed files are re-parsed. A commit
/// never changes, so its index is kept as is (the newest few are kept).
#[derive(Debug, Default)]
pub struct ProjectIndexCache {
    indexes: std::sync::Mutex<CachedIndexes>,
}

impl ProjectIndexCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns an index that reflects the files on disk right now.
    pub fn get_current_project_index(
        &self,
        repo_path: &Path,
    ) -> Result<Arc<ProjectIndex>, AnalysisError> {
        let previous = self.lock_indexes().working_trees.get(repo_path).cloned();
        let index = build_project_index(repo_path, previous.as_ref())?;
        self.lock_indexes()
            .working_trees
            .insert(repo_path.to_path_buf(), Arc::clone(&index));
        Ok(index)
    }

    /// Returns the index of a commit, given its full SHA.
    pub fn get_commit_project_index(
        &self,
        repo_path: &Path,
        commit_sha: &str,
    ) -> Result<Arc<ProjectIndex>, AnalysisError> {
        let key = (repo_path.to_path_buf(), commit_sha.to_string());
        if let Some(index) = self.lock_indexes().commits.get(&key) {
            return Ok(Arc::clone(index));
        }

        let index = Arc::new(build_project_index_at_commit(repo_path, commit_sha)?);
        let mut indexes = self.lock_indexes();
        if indexes
            .commits
            .insert(key.clone(), Arc::clone(&index))
            .is_none()
        {
            indexes.commit_insertion_order.push_back(key);
        }
        while indexes.commit_insertion_order.len() > MAX_COMMIT_INDEXES {
            if let Some(oldest) = indexes.commit_insertion_order.pop_front() {
                indexes.commits.remove(&oldest);
            }
        }
        Ok(index)
    }

    /// Drops every cached index of a repository so the next lookup parses everything again.
    pub fn delete_project_indexes(&self, repo_path: &Path) {
        let mut indexes = self.lock_indexes();
        indexes.working_trees.remove(repo_path);
        indexes.commits.retain(|(path, _), _| path != repo_path);
        indexes
            .commit_insertion_order
            .retain(|(path, _)| path != repo_path);
    }

    fn lock_indexes(&self) -> std::sync::MutexGuard<'_, CachedIndexes> {
        // The maps stay valid even if a holder panicked, so recover the guard.
        self.indexes
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}
