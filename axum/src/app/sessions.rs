//! The sessions API, `app_core::app::sessions::host`: the tmux sessions
//! and git worktrees on the server's machine, and the coding agents in
//! them. It runs programs there, so only an app's bearer token reaches
//! it. Showing a session in a terminal window is for the machine itself:
//! a request that came over the tailnet, which `tailscale serve` marks
//! with `Tailscale-User-Login`, is refused.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use serde::Deserialize;

use app_core::app::server_state::ServerState;
use app_core::app::sessions::summaries::SessionSummaries;
use app_core::app::sessions::{DetectedSettings, SessionStatus};
use app_core::services::agents::chat::ChatHistory;
use app_core::services::terminal_app::ShowOutcome;

use crate::api_error::ApiError;
use crate::user_auth::CurrentUserFromBearerToken;

/// The largest image pasted into a prompt, sent as base64.
pub const PROMPT_IMAGE_MAX_BYTES: usize = 64 * 1024 * 1024;

/// The header `tailscale serve` adds to a request from the tailnet.
const TAILSCALE_USER_LOGIN_HEADER: &str = "tailscale-user-login";

#[derive(Deserialize)]
pub struct NewSession {
    pub name: String,
    pub repository: String,
    pub base_branch: String,
    pub command: String,
    pub port: u16,
}

#[derive(Deserialize)]
pub struct NewPrompt {
    pub prompt: String,
}

#[derive(Deserialize)]
pub struct NewPromptImage {
    pub media_type: String,
    pub data_base64: String,
}

#[derive(Deserialize)]
pub struct NewMerge {
    pub into_branch: String,
}

pub async fn settings_get(
    State(state): State<Arc<ServerState>>,
    CurrentUserFromBearerToken(_user): CurrentUserFromBearerToken,
) -> Json<DetectedSettings> {
    Json(state.sessions.settings_get().await)
}

pub async fn default_port_get(
    State(state): State<Arc<ServerState>>,
    CurrentUserFromBearerToken(_user): CurrentUserFromBearerToken,
) -> Result<Json<u16>, ApiError> {
    Ok(Json(state.sessions.default_port_get().await?))
}

pub async fn list(
    State(state): State<Arc<ServerState>>,
    CurrentUserFromBearerToken(_user): CurrentUserFromBearerToken,
) -> Result<Json<SessionSummaries>, ApiError> {
    Ok(Json(state.sessions.list().await?))
}

pub async fn create(
    State(state): State<Arc<ServerState>>,
    CurrentUserFromBearerToken(_user): CurrentUserFromBearerToken,
    Json(new): Json<NewSession>,
) -> Result<Json<String>, ApiError> {
    let name = state
        .sessions
        .create(
            &new.name,
            &new.repository,
            &new.base_branch,
            &new.command,
            new.port,
        )
        .await?;
    Ok(Json(name))
}

pub async fn get(
    State(state): State<Arc<ServerState>>,
    CurrentUserFromBearerToken(_user): CurrentUserFromBearerToken,
    Path(name): Path<String>,
) -> Result<Json<SessionStatus>, ApiError> {
    Ok(Json(state.sessions.get(&name).await?))
}

pub async fn delete(
    State(state): State<Arc<ServerState>>,
    CurrentUserFromBearerToken(_user): CurrentUserFromBearerToken,
    Path(name): Path<String>,
) -> Result<Json<String>, ApiError> {
    Ok(Json(state.sessions.delete(&name).await?))
}

pub async fn chat_get(
    State(state): State<Arc<ServerState>>,
    CurrentUserFromBearerToken(_user): CurrentUserFromBearerToken,
    Path(name): Path<String>,
) -> Result<Json<ChatHistory>, ApiError> {
    Ok(Json(state.sessions.chat_get(&name).await?))
}

pub async fn prompts_create(
    State(state): State<Arc<ServerState>>,
    CurrentUserFromBearerToken(_user): CurrentUserFromBearerToken,
    Path(name): Path<String>,
    Json(new): Json<NewPrompt>,
) -> Result<Json<()>, ApiError> {
    Ok(Json(
        state.sessions.prompts_create(&name, &new.prompt).await?,
    ))
}

pub async fn prompt_images_create(
    State(state): State<Arc<ServerState>>,
    CurrentUserFromBearerToken(_user): CurrentUserFromBearerToken,
    Path(name): Path<String>,
    Json(new): Json<NewPromptImage>,
) -> Result<Json<String>, ApiError> {
    let path = state
        .sessions
        .prompt_images_create(&name, &new.media_type, &new.data_base64)
        .await?;
    Ok(Json(path))
}

pub async fn continuations_create(
    State(state): State<Arc<ServerState>>,
    CurrentUserFromBearerToken(_user): CurrentUserFromBearerToken,
    Path(name): Path<String>,
) -> Result<Json<ShowOutcome>, ApiError> {
    Ok(Json(state.sessions.continuations_create(&name).await?))
}

pub async fn merges_create(
    State(state): State<Arc<ServerState>>,
    CurrentUserFromBearerToken(_user): CurrentUserFromBearerToken,
    Path(name): Path<String>,
    Json(new): Json<NewMerge>,
) -> Result<Json<String>, ApiError> {
    Ok(Json(
        state
            .sessions
            .merges_create(&name, &new.into_branch)
            .await?,
    ))
}

pub async fn terminal_windows_create(
    State(state): State<Arc<ServerState>>,
    CurrentUserFromBearerToken(_user): CurrentUserFromBearerToken,
    headers: HeaderMap,
    Path(name): Path<String>,
) -> Result<Json<ShowOutcome>, ApiError> {
    if headers.contains_key(TAILSCALE_USER_LOGIN_HEADER) {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "Only the server's own machine shows a session in its terminal",
        ));
    }
    Ok(Json(state.sessions.terminal_windows_create(&name).await?))
}

pub async fn browser_url_get(
    State(state): State<Arc<ServerState>>,
    CurrentUserFromBearerToken(_user): CurrentUserFromBearerToken,
    Path(name): Path<String>,
) -> Result<Json<String>, ApiError> {
    Ok(Json(state.sessions.browser_url_get(&name).await?))
}

pub async fn artifacts_url_get(
    State(state): State<Arc<ServerState>>,
    CurrentUserFromBearerToken(_user): CurrentUserFromBearerToken,
    Path(name): Path<String>,
) -> Result<Json<String>, ApiError> {
    Ok(Json(state.sessions.artifacts_url_get(&name).await?))
}
