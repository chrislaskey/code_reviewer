export { default as App } from "./App";
export { BackendProvider, useBackend } from "./api/context";
export type { BackendAdapter } from "./api/adapter";
export type {
  GreetRequest,
  GreetResponse,
  ChangedFile,
  ListChangedFilesRequest,
  ListChangedFilesResponse,
} from "./api/types";
