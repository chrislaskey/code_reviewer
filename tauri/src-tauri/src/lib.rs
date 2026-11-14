//! The Code Reviewer desktop app: a Tauri shell with three features.
//!
//!   * Reviews (`review`): what a git comparison changed in an Elixir or
//!     Rust repository, function by function, file by file and group by group,
//!     computed by the `code_reviewer_core` crate.
//!   * Traces (`trace`): the tests in the head of a comparison, and the
//!     calls below one test walked top to bottom like a stack trace, each
//!     function marked against the base. Also computed by the core crate.
//!   * Sessions (`sessions`): the tmux sessions the app started, named
//!     `cr-<name>`, with the agent (Claude Code or Cursor) status in each, and its git
//!     worktrees (named the same) without a session. A new session is a
//!     worktree of a repository with a tmux session in it, opened in the
//!     user's terminal app; a worktree's session can be continued, and
//!     finished by merging its branch or closing its worktree.
//!
//! The pages are in `../src`, one HTML entry per page. They call the
//! commands below through `src/lib/backend.js` and nothing else. Each
//! command is one call over HTTP to the backend server the app is connected
//! to (`connection.rs`, decision 25): on the desktop, the server it starts at
//! launch on localhost; on a phone, the computer it was paired with.

mod connection;
mod remote_access;
mod review;
mod sessions;
mod trace;

use tauri::{App, Manager};

use connection::BackendConnection;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::Builder::from_env(
        env_logger::Env::default()
            .default_filter_or("code_reviewer_lib=info,code_reviewer_server=info"),
    )
    .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let connection = connect_to_backend(app)?;
            app.manage(connection);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            review::list_branches_and_recent_commits,
            review::analyze_file_changes,
            review::get_function_diffs,
            review::get_file_diffs,
            trace::list_tests,
            trace::get_test_call_stack,
            sessions::get_session_settings,
            sessions::get_default_session_port,
            sessions::list_sessions,
            sessions::get_session,
            sessions::get_agent_chat_history,
            sessions::send_prompt_to_session,
            sessions::save_prompt_image_for_session,
            sessions::create_session,
            sessions::continue_session,
            sessions::merge_session,
            sessions::close_session,
            sessions::show_session_in_terminal_app,
            sessions::open_session_in_browser,
            sessions::open_session_artifacts_in_browser,
            remote_access::get_app_platform,
            remote_access::get_remote_access_status,
            remote_access::set_remote_access_enabled,
            remote_access::create_paired_device,
            remote_access::revoke_paired_device,
            remote_access::get_pairing_status,
            remote_access::pair_with_computer,
            remote_access::unpair_from_computer,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// On the desktop: starts the backend server on localhost and connects to
/// it. The server is managed too, so it lives as long as the app.
#[cfg(desktop)]
fn connect_to_backend(app: &mut App) -> Result<BackendConnection, Box<dyn std::error::Error>> {
    use code_reviewer_client::BackendClient;
    use code_reviewer_server::ServerConfig;

    let config = ServerConfig::new(app.path().app_data_dir()?);
    let server = tauri::async_runtime::block_on(code_reviewer_server::start(config))?;
    let client = BackendClient::new(server.localhost_url(), server.desktop_token().clone());
    app.manage(server);
    Ok(BackendConnection::to_local_server(client))
}

/// On a phone: connects to the computer it was paired with, if it was.
#[cfg(mobile)]
fn connect_to_backend(app: &mut App) -> Result<BackendConnection, Box<dyn std::error::Error>> {
    let pairing_code_path = app.path().app_data_dir()?.join("pairing_code");
    Ok(BackendConnection::from_saved_pairing_code(
        pairing_code_path,
    ))
}
