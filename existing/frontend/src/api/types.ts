/** Request to greet a person. Mirrors core::GreetRequest. */
export interface GreetRequest {
  name: string;
}

/** Response with greeting message. Mirrors core::GreetResponse. */
export interface GreetResponse {
  message: string;
}

/** Mirrors core::git::ListBranchesAndRecentCommitsRequest. */
export interface ListBranchesAndRecentCommitsRequest {
  repo_path: string;
  /** Defaults to 30. */
  recent_commit_count?: number | null;
}

/** Mirrors core::git::CommitSummary. */
export interface CommitSummary {
  sha: string;
  short_sha: string;
  subject: string;
  /** YYYY-MM-DD. */
  committed_on: string;
}

/** Mirrors core::git::ListBranchesAndRecentCommitsResponse. */
export interface ListBranchesAndRecentCommitsResponse {
  /** Null when HEAD is detached. */
  current_branch: string | null;
  default_branch: string | null;
  /** Local branches first, then remote branches without a local copy. */
  branches: string[];
  /** Newest first. */
  recent_commits: CommitSummary[];
}

/** A single file changed in the git working tree. Mirrors core::git::ChangedFile. */
export interface ChangedFile {
  status: string;
  path: string;
}

/** Request to list changed files. Mirrors core::git::ListChangedFilesRequest. */
export interface ListChangedFilesRequest {
  repo_path: string;
}

/** Response with changed files. Mirrors core::git::ListChangedFilesResponse. */
export interface ListChangedFilesResponse {
  files: ChangedFile[];
}

/** Identifies a function. Mirrors core::analysis::model::FunctionId. */
export interface FunctionId {
  module: string;
  name: string;
  arity: number;
}

export type CallDirection = "incoming_calls" | "outgoing_calls";

/** All calls between two functions. Mirrors core::analysis::project_index::CallLink. */
export interface CallLink {
  /** The function at the other end of the call. */
  function: FunctionId;
  /** False for functions outside the project (standard library, deps). */
  defined_in_project: boolean;
  call_file_path: string;
  call_lines: number[];
  crosses_module_boundary: boolean;
}

/** One-level calls across modules. Mirrors core::analysis::external_calls::ExternalCalls. */
export interface ExternalCalls {
  function: FunctionId;
  calls_in: CallLink[];
  calls_out: CallLink[];
}

export type FunctionChangeKind = "added" | "modified" | "removed" | "renamed";

/** Mirrors core::analysis::function_changes::FunctionChange. */
export interface FunctionChange {
  kind: FunctionChangeKind;
  /** The function in the head, or in the base when removed. */
  function: FunctionId;
  /** The name in the base. Only set when renamed. */
  previous_function: FunctionId | null;
  file_path: string;
  previous_file_path: string | null;
  start_line: number;
  end_line: number;
  /** Changed lines in the head. Empty when removed. */
  changed_lines: number[];
  external_calls: ExternalCalls;
  /** Other-module callers of the old name in the base. Only set when renamed. */
  previous_calls_in: CallLink[];
  /** Other-module callers in the head that still call the old name. */
  stale_callers: CallLink[];
}

/**
 * Neither ref: uncommitted changes against HEAD.
 * base_ref only: the working tree against base_ref.
 * head_ref only: that commit against its parent.
 * Both: head_ref against base_ref.
 * Mirrors core::analysis::service::AnalyzeChangesRequest.
 */
export interface AnalyzeChangesRequest {
  repo_path: string;
  base_ref?: string | null;
  head_ref?: string | null;
  /** With both refs: compare from where head branched off base, like a pull request. */
  compare_from_merge_base?: boolean;
}

/**
 * Patterns name a module (`Shop`), a function (`Enum.reduce`, `Enum.reduce/3`)
 * or a raw prefix (`Shop*`). Ignoring wins over including.
 * Mirrors core::analysis::module_rules::ModuleRules.
 */
export interface ModuleRules {
  /** Highlight matching calls like project calls. */
  include?: string[];
  /** Leave matching callers and calls out. */
  ignore?: string[];
}

/** Mirrors core::analysis::service::AnalyzeFileChangesRequest. */
export interface AnalyzeFileChangesRequest extends AnalyzeChangesRequest {
  module_rules?: ModuleRules;
  /** Which groups to place the changed files in. Without any, every file is in the default group. */
  group_flags?: GroupFlags;
}

export interface SkippedFile {
  path: string;
  reason: string;
}

/** Mirrors core::analysis::service::AnalyzeChangesResponse. */
export interface AnalyzeChangesResponse {
  base_commit: string;
  /** Null when the head is the working tree. */
  head_commit: string | null;
  changes: FunctionChange[];
  skipped_files: SkippedFile[];
}

export type FileChangeStatus = "added" | "modified" | "removed";

/** Mirrors core::analysis::file_changes::FileFunctionChange. */
export interface FileFunctionChange {
  kind: FunctionChangeKind;
  function: FunctionId;
  /** The name in the base. Only set when renamed. */
  previous_function: FunctionId | null;
  /** Unknown for removed and renamed functions. */
  added_line_count: number | null;
  /** Only known for removed functions. */
  removed_line_count: number | null;
}

/** Mirrors core::analysis::file_changes::FileCaller. */
export interface FileCaller {
  function: FunctionId;
  file_path: string;
  line: number;
  /** The changed function in this file that is called. */
  calls: FunctionId;
  /** Still calls a function that was removed or renamed. */
  is_stale: boolean;
  is_also_changed: boolean;
}

/** Mirrors core::analysis::file_changes::FileCallee. */
export interface FileCallee {
  function: FunctionId;
  /** False for the standard library and dependencies. */
  defined_in_project: boolean;
  /** Defined in the project, or matched by an include rule. */
  is_highlighted: boolean;
  /** The changed function in this file that makes the call. */
  from: FunctionId;
  call_count: number;
  is_also_changed: boolean;
  /** The file that defines the function. Only known for functions the project defines. */
  file_path: string | null;
}

/** Mirrors core::analysis::file_changes::FileKind. */
export type FileKind = "parsed" | "unparsed";

/** Mirrors core::analysis::file_changes::FileChange. */
export interface FileChange {
  /** `parsed` has functions and calls. `unparsed` is any other changed file. */
  kind: FileKind;
  /** The path in the base. Only set for an unparsed file that was renamed. */
  previous_path: string | null;
  file_path: string;
  module: string;
  status: FileChangeStatus;
  functions: FileFunctionChange[];
  callers: FileCaller[];
  callees: FileCallee[];
  /** Every group that holds the file. */
  group_ids: string[];
}

/** Mirrors core::analysis::service::AnalyzeFileChangesResponse. */
export interface AnalyzeFileChangesResponse {
  base_commit: string;
  head_commit: string | null;
  /** Sorted by path. */
  files: FileChange[];
  skipped_files: SkippedFile[];
  /** The groups that hold changed files. */
  groups: GroupSummary[];
  /** One row for each group in `groups`. */
  group_rows: GroupRow[];
  /** Rolls up every group. */
  all_changes_row: GroupRow;
}

/** Mirrors core::analysis::service::GetFunctionSourceRequest. */
export interface GetFunctionSourceRequest {
  repo_path: string;
  /** Null or omitted reads the working tree. */
  revision?: string | null;
  function: FunctionId;
}

export interface SourceLine {
  number: number;
  text: string;
}

/** The whole function, not only the changed lines. Mirrors core::analysis::function_source::FunctionSource. */
export interface FunctionSource {
  function: FunctionId;
  file_path: string;
  start_line: number;
  end_line: number;
  lines: SourceLine[];
}

/** Mirrors core::analysis::function_diff::DiffLineKind. */
export type DiffLineKind = "context" | "added" | "removed";

/** Mirrors core::analysis::function_diff::DiffLine. */
export interface DiffLine {
  kind: DiffLineKind;
  /** Null for added lines. */
  old_number: number | null;
  /** Null for removed lines. */
  new_number: number | null;
  text: string;
}

/** The function in the base and in the head. Either is null when it does not exist there. */
export interface FunctionDiffTarget {
  old_function: FunctionId | null;
  new_function: FunctionId | null;
}

/** Mirrors core::analysis::service::GetFunctionDiffsRequest. */
export interface GetFunctionDiffsRequest {
  repo_path: string;
  base_commit: string;
  /** Null or omitted when the head is the working tree. */
  head_commit?: string | null;
  targets: FunctionDiffTarget[];
}

/** The whole function with the changed lines marked. Mirrors core::analysis::function_diff::FunctionDiff. */
export interface FunctionDiff {
  old_function: FunctionId | null;
  new_function: FunctionId | null;
  file_path: string;
  lines: DiffLine[];
}

/** The file in the base and in the head. Either is null when it does not exist there. */
export interface FileDiffTarget {
  old_path: string | null;
  new_path: string | null;
}

/** Mirrors core::analysis::service::GetFileDiffsRequest. */
export interface GetFileDiffsRequest {
  repo_path: string;
  base_commit: string;
  /** Null or omitted when the head is the working tree. */
  head_commit?: string | null;
  targets: FileDiffTarget[];
}

/** The changed lines of a file with context. Mirrors core::analysis::file_diff::FileDiff. */
export interface FileDiff {
  old_path: string | null;
  new_path: string | null;
  /** Empty for a binary file. */
  lines: DiffLine[];
  is_binary: boolean;
  /** Lines left out because the file is very long. */
  omitted_line_count: number;
  /** True for a Markdown file, which also has `html_hunks`. */
  is_markdown: boolean;
  /** The new version of each stretch of changed lines as HTML. Blocks that hold added lines carry a `data-added` attribute. Empty unless Markdown. */
  html_hunks: string[];
}

/** Mirrors core::analysis::service::GetCallTraceRequest. */
export interface GetCallTraceRequest {
  repo_path: string;
  /** Null or omitted traces the working tree. */
  revision?: string | null;
  function: FunctionId;
  direction: CallDirection;
  /** Omitted follows calls until cycles or dead ends. */
  max_depth?: number | null;
  /** Omitted uses the server default of 5000. */
  max_node_count?: number | null;
}

export type TraceStopReason = "cycle" | "max_depth" | "max_node_count";

export interface CallTraceNode {
  function: FunctionId;
  /** False for functions outside the project, which are never expanded. */
  defined_in_project: boolean;
  /** File and lines of the call that led here. Empty on the root. */
  call_file_path: string | null;
  call_lines: number[];
  crosses_module_boundary: boolean;
  stop_reason: TraceStopReason | null;
  children: CallTraceNode[];
}

/** Mirrors core::analysis::call_trace::GetCallTraceResponse. */
export interface GetCallTraceResponse {
  root: CallTraceNode;
  node_count: number;
  /** True when max_depth or max_node_count cut the tree short. */
  truncated: boolean;
}

/**
 * One group. Names are slugs (letters, digits, `_`, `-`). It holds the files matching
 * one of its `include` patterns and none of its `exclude` patterns.
 * Patterns are globs matched against repo-relative paths, or `regex:<expression>`.
 * Mirrors core::groups::GroupDefinition.
 */
export interface GroupDefinition {
  /** Shown instead of the name. */
  label?: string;
  include?: string[];
  exclude?: string[];
}

/** Keys keep their order. Mirrors core::groups::GroupConfig. */
export interface GroupConfig {
  groups: Record<string, GroupDefinition>;
}

/** Group options of the command line. Mirrors core::groups::GroupFlags. */
export interface GroupFlags {
  /** A config file to use instead of `.code_reviewer.toml` in the repo root. */
  group_config_path?: string | null;
  /** Each entry is `<group>=<pattern>`. */
  include?: string[];
  exclude?: string[];
}

/**
 * One group of the ordered list. Ends with the implicit `ungrouped` group.
 * Mirrors core::groups::GroupSummary.
 */
export interface GroupSummary {
  /** The group name in the config. `ungrouped` is the implicit group for the rest. */
  id: string;
  label: string;
}

/** Mirrors core::analysis::group_changes::GroupStats. */
export interface GroupStats {
  file_count: number;
  function_count: number;
  function_counts: Record<FunctionChangeKind, number>;
  added_lines: number;
  removed_lines: number;
  /** Callers still calling something removed or renamed. */
  stale_caller_count: number;
}

/** One line of a group row: a changed file, or a group on the All changes row. */
export interface GroupLine {
  /** The file path, or the group id. */
  key: string;
  kind: "file" | "group";
  id: string;
  /** The file path, or the group label. */
  label: string;
  stats: GroupStats;
  /** Files only. */
  status?: FileChangeStatus;
}

/** A file outside the group on the other end of a call. */
export interface GroupLink {
  id: string;
  side: "input" | "output";
  path: string;
  module: string;
  line_key: string;
  line_label: string;
  /** The first group of the other file. */
  group_id: string;
  group_label: string;
  /** Set when the other file is itself changed in this review. */
  changed_file_path: string | null;
  call_count: number;
  /** Inputs only: still calls something removed or renamed. */
  is_stale: boolean;
}

/** One group drawn as a row. Mirrors core::analysis::group_changes::GroupRow. */
export interface GroupRow {
  group: GroupSummary;
  stats: GroupStats;
  status: FileChangeStatus;
  content_label: "Files" | "Groups";
  lines: GroupLine[];
  outputs: GroupLink[];
  inputs: GroupLink[];
  /** Calls into dependencies and the standard library, left out of the outputs. */
  folded_output_count: number;
}

/** Mirrors core::analysis::test_call_stack::TestSummary. */
export interface TestSummary {
  function: FunctionId;
  file_path: string;
  start_line: number;
  end_line: number;
  /** Guessed from the file path (`/live/`) or module name (`LiveTest`). */
  is_live_view_test: boolean;
  /** Set when the test itself was added or modified in the comparison. */
  change: FunctionChangeKind | null;
}

/** Mirrors core::analysis::service::ListTestsRequest: the same comparison as a review. */
export type ListTestsRequest = AnalyzeChangesRequest;

export interface ListTestsResponse {
  base_commit: string;
  head_commit: string | null;
  /** Every test in the head, by file and then by line. */
  tests: TestSummary[];
}

/** Mirrors core::analysis::test_call_stack::TestCallStackOptions. */
export interface TestCallStackOptions {
  /** Omitted follows calls until every function was shown once. */
  max_depth?: number | null;
  /** Omitted uses the server default of 2000. */
  max_frame_count?: number | null;
}

/** Mirrors core::analysis::service::GetTestCallStackRequest. */
export interface GetTestCallStackRequest extends AnalyzeChangesRequest {
  test: FunctionId;
  options?: TestCallStackOptions;
}

/** Mirrors core::analysis::test_call_stack::CallStackFrame. */
export interface CallStackFrame {
  function: FunctionId;
  /** Zero on the test, one more for each call below it. */
  depth: number;
  /** False for functions outside the project (deps, standard library), which have no source here. */
  defined_in_project: boolean;
  /** Where the function is defined. Null outside the project. */
  file_path: string | null;
  /** File and lines of the call that led here. Empty on the test. */
  call_file_path: string | null;
  call_lines: number[];
  crosses_module_boundary: boolean;
  /** Null when the function reads the same in the base, or is outside the project. */
  change: FunctionChangeKind | null;
  /** The whole function in the head, each line marked against the base. Empty outside the project. */
  lines: DiffLine[];
  /** True when the function was already walked above, so its calls are not listed again. */
  repeated: boolean;
  stop_reason: TraceStopReason | null;
}

/** Mirrors core::analysis::service::GetTestCallStackResponse. */
export interface GetTestCallStackResponse {
  base_commit: string;
  head_commit: string | null;
  test: FunctionId;
  frames: CallStackFrame[];
  /** True when the options cut the walk short. */
  truncated: boolean;
}
