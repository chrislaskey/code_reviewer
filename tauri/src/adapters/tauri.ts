import { invoke } from "@tauri-apps/api/core";
import type {
  AnalyzeChangesRequest,
  AnalyzeChangesResponse,
  AnalyzeFileChangesResponse,
  BackendAdapter,
  FunctionSource,
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

  analyzeFileChanges(
    request: AnalyzeChangesRequest,
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

  getCallTrace(request: GetCallTraceRequest): Promise<GetCallTraceResponse> {
    return invoke<GetCallTraceResponse>("get_call_trace", { request });
  },
};
