import type {
  AnalyzeChangesRequest,
  AnalyzeChangesResponse,
  AnalyzeFileChangesResponse,
  GetFunctionSourceRequest,
  FunctionSource,
  GetCallTraceRequest,
  GetCallTraceResponse,
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

  /** List added, modified, removed and renamed functions with their one-level external calls. */
  analyzeChanges(
    request: AnalyzeChangesRequest,
  ): Promise<AnalyzeChangesResponse>;

  /** Group the same changes by file, with the callers and calls of each file. */
  analyzeFileChanges(
    request: AnalyzeChangesRequest,
  ): Promise<AnalyzeFileChangesResponse>;

  /** Get the whole function, not only the changed lines. */
  getFunctionSource(
    request: GetFunctionSourceRequest,
  ): Promise<FunctionSource>;

  /** Follow every call above or below a function through the whole project. */
  getCallTrace(request: GetCallTraceRequest): Promise<GetCallTraceResponse>;
}
