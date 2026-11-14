//! Running Claude Code and Cursor sessions in tmux (`src/sessions/`): the
//! Tauri commands, each one call to the backend server the app
//! is connected to.

use code_reviewer_server::requests::{CreateSessionRequest, SavePromptImageForSessionRequest};
use code_reviewer_server::{
    ChatHistory, DetectedSettings, SessionList, SessionStatus, ShowOutcome,
};
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use crate::connection::BackendConnection;

/// The detected shell, terminal app and tmux, and where new sessions start.
#[tauri::command]
pub async fn get_session_settings(
    connection: State<'_, BackendConnection>,
) -> Result<DetectedSettings, String> {
    connection
        .client()?
        .get_session_settings()
        .await
        .map_err(|error| error.to_string())
}

/// The port a new session gets unless the user picks one: the lowest free
/// one among the app's running sessions.
#[tauri::command]
pub async fn get_default_session_port(
    connection: State<'_, BackendConnection>,
) -> Result<u16, String> {
    connection
        .client()?
        .get_default_session_port()
        .await
        .map_err(|error| error.to_string())
}

/// The tmux sessions the app started, each with its agent status, and
/// its worktrees without a session.
#[tauri::command]
pub async fn list_sessions(
    connection: State<'_, BackendConnection>,
) -> Result<SessionList, String> {
    connection
        .client()?
        .list_sessions()
        .await
        .map_err(|error| error.to_string())
}

/// One of the app's tmux sessions, with its agent and git status. `name` is
/// the full session name, prefix included.
#[tauri::command]
pub async fn get_session(
    connection: State<'_, BackendConnection>,
    name: String,
) -> Result<SessionStatus, String> {
    connection
        .client()?
        .get_session(&name)
        .await
        .map_err(|error| error.to_string())
}

/// The chat history of the agent in one of the app's sessions: its messages
/// and tool calls, read from the transcript the agent writes.
#[tauri::command]
pub async fn get_agent_chat_history(
    connection: State<'_, BackendConnection>,
    name: String,
) -> Result<ChatHistory, String> {
    connection
        .client()?
        .get_agent_chat_history(&name)
        .await
        .map_err(|error| error.to_string())
}

/// Types `prompt` into the agent's pane in one of the app's sessions and
/// presses Enter, as if the user typed it there.
#[tauri::command]
pub async fn send_prompt_to_session(
    connection: State<'_, BackendConnection>,
    name: String,
    prompt: String,
) -> Result<(), String> {
    connection
        .client()?
        .send_prompt_to_session(&name, &prompt)
        .await
        .map_err(|error| error.to_string())
}

/// Saves an image, sent as base64, in the artifacts of a session's checkout
/// and returns its path, to put in a prompt for its agent.
#[tauri::command]
pub async fn save_prompt_image_for_session(
    connection: State<'_, BackendConnection>,
    name: String,
    media_type: String,
    data_base64: String,
) -> Result<String, String> {
    let request = SavePromptImageForSessionRequest {
        name,
        media_type,
        data_base64,
    };
    connection
        .client()?
        .save_prompt_image_for_session(&request)
        .await
        .map_err(|error| error.to_string())
}

/// Creates a git worktree of `repository` on a branch named `name` and a
/// tmux session named `<prefix>-<name>` in it with `PORT` set to `port`,
/// types `command` into it and opens it in the terminal app. Returns the
/// session's full name, prefix included.
#[tauri::command]
pub async fn create_session(
    connection: State<'_, BackendConnection>,
    name: String,
    repository: String,
    command: String,
    port: u16,
) -> Result<String, String> {
    let request = CreateSessionRequest {
        name,
        repository,
        command,
        port,
    };
    connection
        .client()?
        .create_session(&request)
        .await
        .map_err(|error| error.to_string())
}

/// Starts a tmux session again in one of the app's worktrees, named after
/// it, and opens it in the terminal app. `name` is the worktree's name,
/// prefix included.
#[tauri::command]
pub async fn continue_session(
    connection: State<'_, BackendConnection>,
    name: String,
) -> Result<ShowOutcome, String> {
    connection
        .client()?
        .continue_session(&name)
        .await
        .map_err(|error| error.to_string())
}

/// Merges a session's branch into the main branch (fast-forward only),
/// removes its worktree, deletes the branch and closes its tmux session.
/// `name` is the worktree's name, prefix included. Returns what was done.
#[tauri::command]
pub async fn merge_session(
    connection: State<'_, BackendConnection>,
    name: String,
) -> Result<String, String> {
    connection
        .client()?
        .merge_session(&name)
        .await
        .map_err(|error| error.to_string())
}

/// Removes a session's worktree, keeping its branch, and closes its tmux
/// session. `name` is the worktree's name, prefix included. Returns what was
/// done.
#[tauri::command]
pub async fn close_session(
    connection: State<'_, BackendConnection>,
    name: String,
) -> Result<String, String> {
    connection
        .client()?
        .close_session(&name)
        .await
        .map_err(|error| error.to_string())
}

/// Brings the terminal window of one of the app's sessions to the front, or
/// opens one. `name` is the full session name, prefix included.
#[tauri::command]
pub async fn show_session_in_terminal_app(
    connection: State<'_, BackendConnection>,
    name: String,
) -> Result<ShowOutcome, String> {
    connection
        .client()?
        .show_session_in_terminal_app(&name)
        .await
        .map_err(|error| error.to_string())
}

/// Opens a session's `http://localhost:<port>` in this device's browser:
/// on a phone, at the paired computer's tailnet name. Returns the URL.
#[tauri::command]
pub async fn open_session_in_browser(
    app: AppHandle,
    connection: State<'_, BackendConnection>,
    name: String,
) -> Result<String, String> {
    let url = connection
        .client()?
        .get_session_browser_url(&name)
        .await
        .map_err(|error| error.to_string())?;
    open_url_on_this_device(&app, &connection, &url)
}

/// Opens the artifacts of a session's checkout in this device's browser:
/// on a phone, at the paired computer's tailnet name. Returns the URL.
#[tauri::command]
pub async fn open_session_artifacts_in_browser(
    app: AppHandle,
    connection: State<'_, BackendConnection>,
    name: String,
) -> Result<String, String> {
    let url = connection
        .client()?
        .get_session_artifacts_url(&name)
        .await
        .map_err(|error| error.to_string())?;
    open_url_on_this_device(&app, &connection, &url)
}

fn open_url_on_this_device(
    app: &AppHandle,
    connection: &BackendConnection,
    url_on_server: &str,
) -> Result<String, String> {
    let url = connection.url_on_this_device(url_on_server)?;
    app.opener()
        .open_url(&url, None::<&str>)
        .map_err(|error| format!("Could not open {url}: {error}"))?;
    Ok(url)
}
