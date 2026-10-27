// Every call to the Rust side goes through here, so pages and components
// never call `invoke` themselves. When a web server joins the desktop app,
// this is the one file that changes. The names are the Tauri commands' (see
// src-tauri/src/lib.rs and review.rs); the request and response shapes are
// the core crate's, with snake_case fields.

import { invoke } from "@tauri-apps/api/core";

/* -- Reviews ------------------------------------------------------------------- */

/** The branches and latest commits of a repository, to choose what to review. */
export function listBranchesAndRecentCommits(request) {
  return invoke("list_branches_and_recent_commits", { request });
}

/** What a comparison changed, grouped by file and by group. */
export function analyzeFileChanges(request) {
  return invoke("analyze_file_changes", { request });
}

/** The whole of each changed function, with its added and removed lines marked. */
export function getFunctionDiffs(request) {
  return invoke("get_function_diffs", { request });
}

/** The changed lines of files of any type, with a few lines of context. */
export function getFileDiffs(request) {
  return invoke("get_file_diffs", { request });
}

/* -- Sessions ------------------------------------------------------------------ */

/** The detected shell, terminal app and tmux, and where new sessions start. */
export function getSessionSettings() {
  return invoke("get_session_settings");
}

/** The tmux sessions with what runs in each pane, and the Claude Code sessions. */
export function listSessions() {
  return invoke("list_sessions");
}

/** Creates a tmux session running `command` in `directory` and opens it in the terminal app. */
export function createSession({ name, directory, command }) {
  return invoke("create_session", { name, directory, command });
}

/** Brings a tmux session's terminal window to the front, or opens one. */
export function showSessionInTerminalApp(name) {
  return invoke("show_session_in_terminal_app", { name });
}
