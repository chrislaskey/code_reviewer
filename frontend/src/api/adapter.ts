import type {
  AnalyzeChangesRequest,
  AnalyzeChangesResponse,
  AnalyzeFileChangesRequest,
  AnalyzeFileChangesResponse,
  ListBranchesAndRecentCommitsRequest,
  ListBranchesAndRecentCommitsResponse,
  GetFunctionSourceRequest,
  FunctionSource,
  FunctionDiff,
  GetFunctionDiffsRequest,
  FileDiff,
  GetFileDiffsRequest,
  GetCallTraceRequest,
  GetCallTraceResponse,
  GetTestCallStackRequest,
  GetTestCallStackResponse,
  ListTestsRequest,
  ListTestsResponse,
  GreetRequest,
  GreetResponse,
  ListChangedFilesRequest,
  ListChangedFilesResponse,
} from "./types";

/**
 * Platform-agnostic backend adapter.
 *
 * Each platform (Tauri, Axum) provides its own implementation.
 * Components consume this interface via React context — never
 * calling invoke() or fetch() directly.
 */
export interface BackendAdapter {
  greet(request: GreetRequest): Promise<GreetResponse>;
  listChangedFiles(
    request: ListChangedFilesRequest,
  ): Promise<ListChangedFilesResponse>;

  /** List the branches and latest commits, to choose what to review. */
  listBranchesAndRecentCommits(
    request: ListBranchesAndRecentCommitsRequest,
  ): Promise<ListBranchesAndRecentCommitsResponse>;

  /** List added, modified, removed and renamed functions with their one-level external calls. */
  analyzeChanges(
    request: AnalyzeChangesRequest,
  ): Promise<AnalyzeChangesResponse>;

  /** Group the same changes by file, with the callers and calls of each file. */
  analyzeFileChanges(
    request: AnalyzeFileChangesRequest,
  ): Promise<AnalyzeFileChangesResponse>;

  /** Get the whole function, not only the changed lines. */
  getFunctionSource(
    request: GetFunctionSourceRequest,
  ): Promise<FunctionSource>;

  /** Get whole functions with the lines that changed between the base and the head marked. */
  getFunctionDiffs(request: GetFunctionDiffsRequest): Promise<FunctionDiff[]>;

  /** Get the changed lines of files of any type, with a few lines of context. */
  getFileDiffs(request: GetFileDiffsRequest): Promise<FileDiff[]>;

  /** Follow every call above or below a function through the whole project. */
  getCallTrace(request: GetCallTraceRequest): Promise<GetCallTraceResponse>;

  /** List every test in the head of a comparison. */
  listTests(request: ListTestsRequest): Promise<ListTestsResponse>;

  /** Walk every call below a test, top to bottom, with each project function marked against the base. */
  getTestCallStack(request: GetTestCallStackRequest): Promise<GetTestCallStackResponse>;
}
