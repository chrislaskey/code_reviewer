import type {
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
}
