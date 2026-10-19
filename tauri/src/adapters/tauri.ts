import { invoke } from "@tauri-apps/api/core";
import type {
  AnalyzeChangesRequest,
  AnalyzeChangesResponse,
  AnalyzeFileChangesRequest,
  AnalyzeFileChangesResponse,
  ListBranchesAndRecentCommitsRequest,
  ListBranchesAndRecentCommitsResponse,
  BackendAdapter,
  FunctionDiff,
  FunctionSource,
  GetFunctionDiffsRequest,
  FileDiff,
  GetFileDiffsRequest,
  GetCallTraceRequest,
  GetCallTraceResponse,
  GetFunctionSourceRequest,
  ChangedFile,
  GreetRequest,
  GreetResponse,
  ListChangedFilesRequest,
  ListChangedFilesResponse,
} from "@code-reviewer/ui";

/** BackendAdapter that calls Rust via Tauri's invoke bridge. */
export const tauriAdapter: BackendAdapter = {
  async greet(request: GreetRequest): Promise<GreetResponse> {
    const message = await invoke<string>("greet", { name: request.name });
    return { message };
  },

  async listChangedFiles(
    request: ListChangedFilesRequest,
  ): Promise<ListChangedFilesResponse> {
    const files = await invoke<ChangedFile[]>("list_changed_files", {
      repoPath: request.repo_path,
    });
    return { files };
  },

  analyzeChanges(
    request: AnalyzeChangesRequest,
  ): Promise<AnalyzeChangesResponse> {
    return invoke<AnalyzeChangesResponse>("analyze_changes", { request });
  },

  listBranchesAndRecentCommits(
    request: ListBranchesAndRecentCommitsRequest,
  ): Promise<ListBranchesAndRecentCommitsResponse> {
    return invoke<ListBranchesAndRecentCommitsResponse>(
      "list_branches_and_recent_commits",
      { request },
    );
  },

  analyzeFileChanges(
    request: AnalyzeFileChangesRequest,
  ): Promise<AnalyzeFileChangesResponse> {
    return invoke<AnalyzeFileChangesResponse>("analyze_file_changes", {
      request,
    });
  },

  getFunctionSource(
    request: GetFunctionSourceRequest,
  ): Promise<FunctionSource> {
    return invoke<FunctionSource>("get_function_source", { request });
  },

  getFunctionDiffs(request: GetFunctionDiffsRequest): Promise<FunctionDiff[]> {
    return invoke<FunctionDiff[]>("get_function_diffs", { request });
  },

  getFileDiffs(request: GetFileDiffsRequest): Promise<FileDiff[]> {
    return invoke<FileDiff[]>("get_file_diffs", { request });
  },

  getCallTrace(request: GetCallTraceRequest): Promise<GetCallTraceResponse> {
    return invoke<GetCallTraceResponse>("get_call_trace", { request });
  },
};
