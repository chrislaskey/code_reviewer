import type {
  AnalyzeChangesRequest,
  AnalyzeChangesResponse,
  BackendAdapter,
  FunctionSource,
  GetCallTraceRequest,
  GetCallTraceResponse,
  GetFunctionSourceRequest,
  GreetRequest,
  GreetResponse,
  ListChangedFilesRequest,
  ListChangedFilesResponse,
} from "@code-reviewer/ui";

const API_BASE = "/api";

/** POST a JSON request and read the JSON response. The server's error text is kept. */
async function postJson<Response>(
  path: string,
  request: unknown,
  label: string,
): Promise<Response> {
  const response = await fetch(`${API_BASE}/${path}`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(request),
  });
  if (!response.ok) {
    const detail = await response.text();
    throw new Error(`${label} failed: ${detail || response.statusText}`);
  }
  return response.json();
}

/** BackendAdapter that calls the Axum server via HTTP fetch. */
export const httpAdapter: BackendAdapter = {
  async greet(request: GreetRequest): Promise<GreetResponse> {
    const response = await fetch(`${API_BASE}/greet`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(request),
    });
    if (!response.ok) {
      throw new Error(`Greet request failed: ${response.statusText}`);
    }
    return response.json();
  },

  async listChangedFiles(
    request: ListChangedFilesRequest,
  ): Promise<ListChangedFilesResponse> {
    const response = await fetch(`${API_BASE}/list_changed_files`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(request),
    });
    if (!response.ok) {
      throw new Error(`List changed files failed: ${response.statusText}`);
    }
    return response.json();
  },

  analyzeChanges(
    request: AnalyzeChangesRequest,
  ): Promise<AnalyzeChangesResponse> {
    return postJson("analyze_changes", request, "Analyze changes");
  },

  getFunctionSource(
    request: GetFunctionSourceRequest,
  ): Promise<FunctionSource> {
    return postJson("get_function_source", request, "Get function source");
  },

  getCallTrace(request: GetCallTraceRequest): Promise<GetCallTraceResponse> {
    return postJson("get_call_trace", request, "Get call trace");
  },
};
