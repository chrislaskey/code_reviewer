//! The bodies of the HTTP requests whose command takes more than a core
//! request type. Shared with the client, so both ends agree on the fields.

use serde::{Deserialize, Serialize};

/// Names one of the app's sessions or worktrees, prefix included.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionNameRequest {
    pub name: String,
}

/// What to type into the agent's pane in one of the app's sessions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendPromptToSessionRequest {
    pub name: String,
    pub prompt: String,
}

/// An image to save for a prompt to the agent in one of the app's sessions:
/// its type, like `image/png`, and its bytes as base64.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavePromptImageForSessionRequest {
    pub name: String,
    pub media_type: String,
    pub data_base64: String,
}

/// A new session: a worktree of `repository` on a branch named `name`, with
/// `command` typed into its tmux session and `PORT` set to `port`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSessionRequest {
    pub name: String,
    pub repository: String,
    pub command: String,
    pub port: u16,
}

/// Turns remote access on or off.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetRemoteAccessEnabledRequest {
    pub enabled: bool,
}

/// Pairs a device for remote access.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePairedDeviceRequest {
    pub name: String,
}

/// Names a paired device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairedDeviceIdRequest {
    pub id: String,
}
