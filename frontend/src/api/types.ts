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
