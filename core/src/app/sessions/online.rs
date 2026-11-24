//! Sessions from a device: each asked of the server's API as the signed-in
//! person, its JSON answer passed back as it came, so the app needs none
//! of tmux, git or the agents. Online only, as `accounts::online` is.

use reqwest::Method;
use serde_json::Value;

use crate::accounts::online::OnlineError;
use crate::app::users;
use crate::deployment::Deployment;
use crate::services::web_server::Http;

async fn ask(
    deployment: &Deployment,
    server: &Http,
    method: Method,
    path: &str,
    body: Option<&Value>,
) -> Result<Value, OnlineError> {
    let (_user, token) = users::signed_in_with_token(deployment).await?;
    Ok(server.json(method, &token, path, body).await?)
}

/// A session's path, its name escaped as one path segment: form escaping,
/// with a space as `%20`, since a `+` in a path is a plus.
fn session_path(name: &str, rest: &str) -> String {
    let name = url::form_urlencoded::byte_serialize(name.as_bytes())
        .collect::<String>()
        .replace('+', "%20");
    format!("/api/sessions/{name}{rest}")
}

/// The detected shell, terminal app and tmux, and where new sessions start.
pub async fn settings_get(deployment: &Deployment, server: &Http) -> Result<Value, OnlineError> {
    let path = "/api/sessions/settings";
    ask(deployment, server, Method::GET, path, None).await
}

/// The port a new session gets unless the person picks one.
pub async fn default_port_get(
    deployment: &Deployment,
    server: &Http,
) -> Result<Value, OnlineError> {
    let path = "/api/sessions/default-port";
    ask(deployment, server, Method::GET, path, None).await
}

/// The sessions and the worktrees without one.
pub async fn list(deployment: &Deployment, server: &Http) -> Result<Value, OnlineError> {
    ask(deployment, server, Method::GET, "/api/sessions", None).await
}

/// A new session: `request` has its name, repository, base branch,
/// command and port. Answers its full name.
pub async fn create(
    deployment: &Deployment,
    server: &Http,
    request: &Value,
) -> Result<Value, OnlineError> {
    ask(
        deployment,
        server,
        Method::POST,
        "/api/sessions",
        Some(request),
    )
    .await
}

/// One session, with its agent and git status.
pub async fn get(deployment: &Deployment, server: &Http, name: &str) -> Result<Value, OnlineError> {
    let path = session_path(name, "");
    ask(deployment, server, Method::GET, &path, None).await
}

/// Closes a session, removing its worktree and keeping its branch.
pub async fn delete(
    deployment: &Deployment,
    server: &Http,
    name: &str,
) -> Result<Value, OnlineError> {
    let path = session_path(name, "");
    ask(deployment, server, Method::DELETE, &path, None).await
}

/// The chat of the agent in a session.
pub async fn chat_get(
    deployment: &Deployment,
    server: &Http,
    name: &str,
) -> Result<Value, OnlineError> {
    let path = session_path(name, "/chat");
    ask(deployment, server, Method::GET, &path, None).await
}

/// Types a prompt into the agent's pane: `request` is `{prompt}`.
pub async fn prompts_create(
    deployment: &Deployment,
    server: &Http,
    name: &str,
    request: &Value,
) -> Result<Value, OnlineError> {
    let path = session_path(name, "/prompts");
    ask(deployment, server, Method::POST, &path, Some(request)).await
}

/// Saves an image for a prompt: `request` is `{media_type, data_base64}`.
/// Answers its path.
pub async fn prompt_images_create(
    deployment: &Deployment,
    server: &Http,
    name: &str,
    request: &Value,
) -> Result<Value, OnlineError> {
    let path = session_path(name, "/prompt-images");
    ask(deployment, server, Method::POST, &path, Some(request)).await
}

/// Starts a session again in its worktree.
pub async fn continuations_create(
    deployment: &Deployment,
    server: &Http,
    name: &str,
) -> Result<Value, OnlineError> {
    let path = session_path(name, "/continuation");
    ask(deployment, server, Method::POST, &path, None).await
}

/// Merges a session's branch: `request` is `{into_branch}`.
pub async fn merges_create(
    deployment: &Deployment,
    server: &Http,
    name: &str,
    request: &Value,
) -> Result<Value, OnlineError> {
    let path = session_path(name, "/merge");
    ask(deployment, server, Method::POST, &path, Some(request)).await
}

/// Shows a session in the terminal app of the server's machine.
pub async fn terminal_windows_create(
    deployment: &Deployment,
    server: &Http,
    name: &str,
) -> Result<Value, OnlineError> {
    let path = session_path(name, "/terminal-window");
    ask(deployment, server, Method::POST, &path, None).await
}

/// A session's `http://localhost:<port>`.
pub async fn browser_url_get(
    deployment: &Deployment,
    server: &Http,
    name: &str,
) -> Result<Value, OnlineError> {
    let path = session_path(name, "/browser-url");
    ask(deployment, server, Method::GET, &path, None).await
}

/// The address of a session's artifacts.
pub async fn artifacts_url_get(
    deployment: &Deployment,
    server: &Http,
    name: &str,
) -> Result<Value, OnlineError> {
    let path = session_path(name, "/artifacts-url");
    ask(deployment, server, Method::GET, &path, None).await
}

/// Whether this device's bell is on for a session.
pub async fn bell_get(
    deployment: &Deployment,
    server: &Http,
    name: &str,
) -> Result<Value, OnlineError> {
    let path = session_path(name, "/bell");
    ask(deployment, server, Method::GET, &path, None).await
}

/// Turns this device's bell for a session on or off.
pub async fn bell_update(
    deployment: &Deployment,
    server: &Http,
    name: &str,
    enabled: bool,
) -> Result<Value, OnlineError> {
    let path = session_path(name, "/bell");
    let body = serde_json::json!({ "enabled": enabled });
    ask(deployment, server, Method::PUT, &path, Some(&body)).await
}

/// The notifications waiting for this device, else the next ones within
/// 25 seconds, else none.
pub async fn notifications_waiting_list(
    deployment: &Deployment,
    server: &Http,
) -> Result<Value, OnlineError> {
    let path = "/api/notifications/waiting";
    ask(deployment, server, Method::GET, path, None).await
}

/// Keeps this phone's Apple push token on the server.
pub async fn apple_push_token_update(
    deployment: &Deployment,
    server: &Http,
    token: &str,
) -> Result<Value, OnlineError> {
    let path = "/api/devices/current/apple-push-token";
    let body = serde_json::json!({ "token": token });
    ask(deployment, server, Method::PUT, path, Some(&body)).await
}
