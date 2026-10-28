// Every call to the Rust side goes through here, so pages and components
// never call `invoke` themselves. When a web server joins the desktop app,
// this is the one file that changes. The names are the Tauri commands' (see
// src-tauri/src/lib.rs, review.rs and trace.rs); the request and response shapes are
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

/* -- Traces -------------------------------------------------------------------- */

/** Every test in the head of a comparison, marking the ones it added or modified. */
export function listTests(request) {
  return invoke("list_tests", { request });
}

/** The calls below one test, walked top to bottom, each function marked against the base. */
export function getTestCallStack(request) {
  return invoke("get_test_call_stack", { request });
}

/* -- Sessions ------------------------------------------------------------------ */

/** The detected shell, terminal app and tmux, and where new sessions start. */
export function getSessionSettings() {
  return invoke("get_session_settings");
}

/** The port a new session gets unless the user picks one: the lowest free one among this app's running sessions. */
export function getDefaultSessionPort() {
  return invoke("get_default_session_port");
}

/**
 * The tmux sessions this app started (named with its prefix), each with its Claude Code status, and its git worktrees
 * with no session running.
 */
export function listSessions() {
  return invoke("list_sessions");
}

/**
 * Creates a git worktree of `repository` and a tmux session in it, both named `<prefix>-<name>`, with `PORT` set to
 * `port` and `command` typed in, and opens it in the terminal app.
 */
export function createSession({ name, repository, command, port }) {
  return invoke("create_session", { name, repository, command, port });
}

/** Starts a tmux session again in one of the app's worktrees, named after it, and opens it in the terminal app. */
export function continueSession(name) {
  return invoke("continue_session", { name });
}

/**
 * Merges a session's branch into the main branch (fast-forward only), removes its worktree, deletes the branch and
 * closes its tmux session. Resolves to a sentence saying what was done.
 */
export function mergeSession(name) {
  return invoke("merge_session", { name });
}

/** Removes a session's worktree, keeping its branch, and closes its tmux session. Resolves to what was done. */
export function closeSession(name) {
  return invoke("close_session", { name });
}

/** Brings the terminal window of one of the app's sessions to the front, or opens one. `name` includes the prefix. */
export function showSessionInTerminalApp(name) {
  return invoke("show_session_in_terminal_app", { name });
}

/** Opens a session's `http://localhost:<port>` in the browser. Returns the URL. */
export function openSessionInBrowser(name) {
  return invoke("open_session_in_browser", { name });
}
