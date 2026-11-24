//! The sessions as the server's machine runs them, `SessionHost`: what
//! the routes call, one method each, with the state they share kept in
//! `ServerState` for as long as the server is up.

use std::sync::Arc;

use thiserror::Error;
use tokio::sync::OnceCell;

use crate::app::sessions::artifacts::ArtifactsServer;
use crate::app::sessions::summaries::{SessionSummaries, SessionSummaryCache};
use crate::app::sessions::{self, DetectedSettings, SessionStatus, Settings};
use crate::services::agents::chat::ChatHistory;
use crate::services::agents::recent_writes::OpenFileCache;
use crate::services::terminal_app::ShowOutcome;

/// Why a session could not be listed, made or changed, as the tools on
/// the server's machine said.
#[derive(Debug, Error)]
#[error("{0}")]
pub struct SessionError(pub String);

/// The sessions' shared state.
pub struct SessionHost {
    /// The shell, terminal app and tmux, detected on first use.
    settings: OnceCell<DetectedSettings>,
    /// The sessions list, served at once and refreshed behind it.
    summaries: SessionSummaryCache,
    /// The file each Cursor or Codex process writes, so `lsof` isn't run
    /// on every refresh.
    open_files: Arc<OpenFileCache>,
    /// Where bin/dev keeps and serves artifacts; `None` when it didn't
    /// start the server in the main checkout.
    artifacts: Option<ArtifactsServer>,
}

impl Default for SessionHost {
    fn default() -> SessionHost {
        let open_files = Arc::new(OpenFileCache::new());
        SessionHost {
            settings: OnceCell::new(),
            summaries: SessionSummaryCache::new(Arc::clone(&open_files)),
            open_files,
            artifacts: ArtifactsServer::from_environment(),
        }
    }
}

impl SessionHost {
    async fn detected_settings(&self) -> &DetectedSettings {
        self.settings.get_or_init(sessions::detect_settings).await
    }

    async fn settings(&self) -> &Settings {
        &self.detected_settings().await.settings
    }

    /// The detected shell, terminal app and tmux, and where new sessions start.
    pub async fn settings_get(&self) -> DetectedSettings {
        self.detected_settings().await.clone()
    }

    /// The port a new session gets unless the user picks one: the lowest
    /// free one among the running sessions.
    pub async fn default_port_get(&self) -> Result<u16, SessionError> {
        sessions::get_default_session_port(self.settings().await)
            .await
            .map_err(SessionError)
    }

    /// The tmux sessions the app started, each with its agent status, and
    /// its worktrees without a session, as found up to a few seconds ago.
    pub async fn list(&self) -> Result<SessionSummaries, SessionError> {
        self.summaries
            .list_session_summaries(self.settings().await)
            .await
            .map_err(SessionError)
    }

    /// One session, with its agent and git status. `name` is the full
    /// session name, prefix included.
    pub async fn get(&self, name: &str) -> Result<SessionStatus, SessionError> {
        sessions::get_session(self.settings().await, &self.open_files, name)
            .await
            .map_err(SessionError)
    }

    /// A git worktree of `repository` on a branch named `name`, from
    /// `base_branch` unless the branch exists, and a tmux session in it
    /// with `PORT` set to `port`, `command` typed into it and opened in the
    /// terminal app. Answers the session's full name.
    pub async fn create(
        &self,
        name: &str,
        repository: &str,
        base_branch: &str,
        command: &str,
        port: u16,
    ) -> Result<String, SessionError> {
        let created = sessions::create_session(
            self.settings().await,
            name,
            repository,
            base_branch,
            command,
            port,
        )
        .await;
        self.summaries.forget().await;
        created.map_err(SessionError)
    }

    /// Removes a session's worktree, keeping its branch, and closes its
    /// tmux session. Answers what was done.
    pub async fn delete(&self, name: &str) -> Result<String, SessionError> {
        let closed =
            sessions::close_session(self.settings().await, self.artifacts.as_ref(), name).await;
        self.summaries.forget().await;
        closed.map_err(SessionError)
    }

    /// The chat of the agent in a session, read from the transcript it writes.
    pub async fn chat_get(&self, name: &str) -> Result<ChatHistory, SessionError> {
        sessions::get_agent_chat_history(self.settings().await, &self.open_files, name)
            .await
            .map_err(SessionError)
    }

    /// Types `prompt` into the agent's pane and presses Enter.
    pub async fn prompts_create(&self, name: &str, prompt: &str) -> Result<(), SessionError> {
        sessions::send_prompt_to_session(self.settings().await, &self.open_files, name, prompt)
            .await
            .map_err(SessionError)
    }

    /// Saves an image, sent as base64, in the artifacts of a session's
    /// checkout, and answers its path, to put in a prompt.
    pub async fn prompt_images_create(
        &self,
        name: &str,
        media_type: &str,
        data_base64: &str,
    ) -> Result<String, SessionError> {
        sessions::save_prompt_image_for_session(
            self.settings().await,
            self.artifacts.as_ref(),
            name,
            media_type,
            data_base64,
        )
        .await
        .map_err(SessionError)
    }

    /// Starts a tmux session again in a worktree, named after it, and opens
    /// it in the terminal app.
    pub async fn continuations_create(&self, name: &str) -> Result<ShowOutcome, SessionError> {
        let continued = sessions::continue_session(self.settings().await, name).await;
        self.summaries.forget().await;
        continued.map_err(SessionError)
    }

    /// Merges a session's branch into `into_branch` (fast-forward only),
    /// removes its worktree, deletes the branch and closes its tmux
    /// session. Answers what was done.
    pub async fn merges_create(
        &self,
        name: &str,
        into_branch: &str,
    ) -> Result<String, SessionError> {
        let merged = sessions::merge_session(
            self.settings().await,
            self.artifacts.as_ref(),
            name,
            into_branch,
        )
        .await;
        self.summaries.forget().await;
        merged.map_err(SessionError)
    }

    /// Brings a session's terminal window to the front, or opens one.
    pub async fn terminal_windows_create(&self, name: &str) -> Result<ShowOutcome, SessionError> {
        sessions::show_session_in_terminal_app(self.settings().await, name)
            .await
            .map_err(SessionError)
    }

    /// A session's `http://localhost:<port>`, to open in a browser.
    pub async fn browser_url_get(&self, name: &str) -> Result<String, SessionError> {
        sessions::get_session_browser_url(self.settings().await, name)
            .await
            .map_err(SessionError)
    }

    /// The address of a session's artifacts on the dev server; an error
    /// when bin/dev didn't start the server in the main checkout.
    pub async fn artifacts_url_get(&self, name: &str) -> Result<String, SessionError> {
        sessions::get_session_artifacts_url(self.settings().await, self.artifacts.as_ref(), name)
            .await
            .map_err(SessionError)
    }
}
