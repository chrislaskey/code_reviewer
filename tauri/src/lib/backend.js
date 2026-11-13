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
 * The tmux sessions this app started (named with its prefix), each with its agent status, and its git worktrees
 * with no session running.
 */
export function listSessions() {
  return invoke("list_sessions");
}

/** One of the app's tmux sessions, with its agent and git status, as `listSessions` gives it. `name` includes the prefix. */
export function getSession(name) {
  return invoke("get_session", { name });
}

/**
 * The chat history of the agent in one of the app's sessions: `{ path, messages }`, each message with its role and
 * parts (text as HTML, thinking, and tool calls with their results). Only Claude Code's is read so far.
 */
export function getAgentChatHistory(name) {
  return invoke("get_agent_chat_history", { name });
}

/** Types `prompt` into the pane the agent of one of the app's sessions runs in, and presses Enter. */
export function sendPromptToSession(name, prompt) {
  return invoke("send_prompt_to_session", { name, prompt });
}

/**
 * Creates a git worktree of `repository` and a tmux session in it, both named `<prefix>-<name>`, with `PORT` set to
 * `port` and `command` typed in, and opens it in the terminal app. Returns the session's full name, prefix included.
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

/** Opens a session's `http://localhost:<port>` in this device's browser, at the computer's tailnet name on a phone. Returns the URL. */
export function openSessionInBrowser(name) {
  return invoke("open_session_in_browser", { name });
}

/** Opens the artifacts of a session's checkout in this device's browser, as openSessionInBrowser does. Returns the URL. */
export function openSessionArtifactsInBrowser(name) {
  return invoke("open_session_artifacts_in_browser", { name });
}

/* -- Remote access ------------------------------------------------------------- */

/** "desktop" when this app runs the backend itself, "mobile" when it reaches a paired computer. */
export function getAppPlatform() {
  return invoke("get_app_platform");
}

/** Desktop: whether remote access is on, the URL paired devices use, why it isn't reachable, and the paired devices. */
export function getRemoteAccessStatus() {
  return invoke("get_remote_access_status");
}

/** Desktop: turns remote access on or off. Resolves to the new status. */
export function setRemoteAccessEnabled(enabled) {
  return invoke("set_remote_access_enabled", { enabled });
}

/** Desktop: pairs a device named `name`. Resolves to `{ device, pairing_code }`; the code is shown this once. */
export function createPairedDevice(name) {
  return invoke("create_paired_device", { name });
}

/** Desktop: unpairs a device; its code stops working at once. Resolves to the new status. */
export function revokePairedDevice(id) {
  return invoke("revoke_paired_device", { id });
}

/** Mobile: `{ url }`, the computer this phone is paired with, or a null url. */
export function getPairingStatus() {
  return invoke("get_pairing_status");
}

/** Mobile: pairs this phone with the computer in a pairing code, after checking the computer accepts it. */
export function pairWithComputer(code) {
  return invoke("pair_with_computer", { code });
}

/** Mobile: forgets the computer this phone is paired with. */
export function unpairFromComputer() {
  return invoke("unpair_from_computer");
}
