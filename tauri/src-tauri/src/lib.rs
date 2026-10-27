//! The Code Reviewer desktop app: a Tauri shell with two features.
//!
//!   * Reviews (`review`): what a git comparison changed in an Elixir
//!     repository, function by function, file by file and group by group,
//!     computed by the `code_reviewer_core` crate.
//!   * Sessions (`sessions`): the tmux sessions the app started, named
//!     `cr-<name>`, with the Claude Code status in each. A new session is a
//!     git worktree of a repository with a tmux session in it, opened in
//!     the user's terminal app.
//!
//! The pages are in `../src`, one HTML entry per page. They call the
//! commands below through `src/lib/backend.js` and nothing else.

mod claude_code;
mod git_worktree;
mod process;
mod review;
mod sessions;
mod terminal_app;
mod tmux;

use std::sync::Arc;

use code_reviewer_core::analysis::service::AnalysisService;
use tauri::{Manager, State};
use tokio::sync::OnceCell;

use sessions::{DetectedSettings, SessionStatus};
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
            get_session_settings,
            list_sessions,
            create_session,
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

/// The tmux sessions the app started, each with its Claude Code status.
#[tauri::command]
async fn list_sessions(state: State<'_, AppState>) -> Result<Vec<SessionStatus>, String> {
    let settings = state.session_settings().await;
    sessions::list_sessions(&settings.settings).await
}

/// Creates a git worktree of `repository` on a branch named `name` and a
/// tmux session named `<prefix>-<name>` in it, types `command` into it and
/// opens it in the terminal app.
#[tauri::command]
async fn create_session(
    state: State<'_, AppState>,
    name: String,
    repository: String,
    command: String,
) -> Result<ShowOutcome, String> {
    let settings = state.session_settings().await;
    sessions::create_session(&settings.settings, &name, &repository, &command).await
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
