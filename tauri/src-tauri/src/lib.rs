//! The Code Reviewer desktop app: a Tauri shell with three features.
//!
//!   * Reviews (`review`): what a git comparison changed in an Elixir or
//!     Rust repository, function by function, file by file and group by group,
//!     computed by the `code_reviewer_core` crate.
//!   * Traces (`trace`): the tests in the head of a comparison, and the
//!     calls below one test walked top to bottom like a stack trace, each
//!     function marked against the base. Also computed by the core crate.
//!   * Sessions (`sessions`): the tmux sessions the app started, named
//!     `cr-<name>`, with the Claude Code status in each, and its git
//!     worktrees (named the same) without a session. A new session is a
//!     worktree of a repository with a tmux session in it, opened in the
//!     user's terminal app; a worktree's session can be continued, and
//!     finished by merging its branch or closing its worktree.
//!
//! The pages are in `../src`, one HTML entry per page. They call the
//! commands below through `src/lib/backend.js` and nothing else.

mod analysis;
mod claude_code;
mod git;
mod git_worktree;
mod process;
mod review;
mod sessions;
mod terminal_app;
mod tmux;
mod trace;

use std::sync::Arc;

use code_reviewer_core::analysis::service::AnalysisService;
use tauri::{Manager, State};
use tokio::sync::OnceCell;

use sessions::{DetectedSettings, SessionList};
use terminal_app::ShowOutcome;

/// What the commands share, managed by Tauri.
pub struct AppState {
    /// Holds the project index cache, so one instance lives for the whole app.
    pub(crate) analysis: Arc<AnalysisService>,
    /// The shell, terminal app and tmux, detected on first use.
    pub(crate) session_settings: OnceCell<DetectedSettings>,
}

impl AppState {
    async fn session_settings(&self) -> &DetectedSettings {
        self.session_settings
            .get_or_init(sessions::detect_settings)
            .await
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("code_reviewer_lib=info"),
    )
    .init();

    tauri::Builder::default()
        .setup(|app| {
            app.manage(AppState {
                analysis: Arc::new(AnalysisService::new()),
                session_settings: OnceCell::new(),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            review::list_branches_and_recent_commits,
            review::analyze_file_changes,
            review::get_function_diffs,
            review::get_file_diffs,
            trace::list_tests,
            trace::get_test_call_stack,
            get_session_settings,
            get_default_session_port,
            list_sessions,
            create_session,
            continue_session,
            merge_session,
            close_session,
            show_session_in_terminal_app,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// The detected shell, terminal app and tmux, and where new sessions start.
#[tauri::command]
async fn get_session_settings(state: State<'_, AppState>) -> Result<DetectedSettings, String> {
    Ok(state.session_settings().await.clone())
}

/// The port a new session gets unless the user picks one: the lowest free
/// one among the app's running sessions.
#[tauri::command]
async fn get_default_session_port(state: State<'_, AppState>) -> Result<u16, String> {
    let settings = state.session_settings().await;
    sessions::get_default_session_port(&settings.settings).await
}

/// The tmux sessions the app started, each with its Claude Code status, and
/// its worktrees without a session.
#[tauri::command]
async fn list_sessions(state: State<'_, AppState>) -> Result<SessionList, String> {
    let settings = state.session_settings().await;
    sessions::list_sessions(&settings.settings).await
}

/// Creates a git worktree of `repository` on a branch named `name` and a
/// tmux session named `<prefix>-<name>` in it with `PORT` set to `port`,
/// types `command` into it and opens it in the terminal app.
#[tauri::command]
async fn create_session(
    state: State<'_, AppState>,
    name: String,
    repository: String,
    command: String,
    port: u16,
) -> Result<ShowOutcome, String> {
    let settings = state.session_settings().await;
    sessions::create_session(&settings.settings, &name, &repository, &command, port).await
}

/// Starts a tmux session again in one of the app's worktrees, named after
/// it, and opens it in the terminal app. `name` is the worktree's name,
/// prefix included.
#[tauri::command]
async fn continue_session(state: State<'_, AppState>, name: String) -> Result<ShowOutcome, String> {
    let settings = state.session_settings().await;
    sessions::continue_session(&settings.settings, &name).await
}

/// Merges a session's branch into the main branch (fast-forward only),
/// removes its worktree, deletes the branch and closes its tmux session.
/// `name` is the worktree's name, prefix included. Returns what was done.
#[tauri::command]
async fn merge_session(state: State<'_, AppState>, name: String) -> Result<String, String> {
    let settings = state.session_settings().await;
    sessions::merge_session(&settings.settings, &name).await
}

/// Removes a session's worktree, keeping its branch, and closes its tmux
/// session. `name` is the worktree's name, prefix included. Returns what was
/// done.
#[tauri::command]
async fn close_session(state: State<'_, AppState>, name: String) -> Result<String, String> {
    let settings = state.session_settings().await;
    sessions::close_session(&settings.settings, &name).await
}

/// Brings the terminal window of one of the app's sessions to the front, or
/// opens one. `name` is the full session name, prefix included.
#[tauri::command]
async fn show_session_in_terminal_app(
    state: State<'_, AppState>,
    name: String,
) -> Result<ShowOutcome, String> {
    let settings = state.session_settings().await;
    sessions::show_session_in_terminal_app(&settings.settings, &name).await
}
