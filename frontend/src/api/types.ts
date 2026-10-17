/** Request to greet a person. Mirrors core::GreetRequest. */
export interface GreetRequest {
  name: string;
}

/** Response with greeting message. Mirrors core::GreetResponse. */
export interface GreetResponse {
  message: string;
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
  /** The changed function in this file that makes the call. */
  from: FunctionId;
  call_count: number;
  is_also_changed: boolean;
}

/** Mirrors core::analysis::file_changes::FileChange. */
export interface FileChange {
  file_path: string;
  module: string;
  status: FileChangeStatus;
  functions: FileFunctionChange[];
  callers: FileCaller[];
  callees: FileCallee[];
}

/** Mirrors core::analysis::service::AnalyzeFileChangesResponse. */
export interface AnalyzeFileChangesResponse {
  base_commit: string;
  head_commit: string | null;
  /** Sorted by path. */
  files: FileChange[];
  skipped_files: SkippedFile[];
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
