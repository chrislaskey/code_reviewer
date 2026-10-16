import type {
  BackendAdapter,
  GreetRequest,
  GreetResponse,
  ListChangedFilesRequest,
  ListChangedFilesResponse,
} from "@code-reviewer/ui";

const API_BASE = "/api";

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
};
