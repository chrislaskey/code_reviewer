import { invoke } from "@tauri-apps/api/core";
import type {
  BackendAdapter,
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
};
