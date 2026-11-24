//! The sessions as the server's machine runs them, `SessionHost`: what
//! the routes call, one method each, with the state they share kept in
//! `ServerState` for as long as the server is up. A device, for its bells,
//! the notifications it collects and its push token, is the hash of its
//! sign-in's token; its bells are kept in the server's data folder,
//! `DATA_DIRECTORY`.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use thiserror::Error;
use tokio::sync::OnceCell;

use crate::app::sessions::artifacts::ArtifactsServer;
use crate::app::sessions::notifications::{SessionNotification, SessionNotifications};
use crate::app::sessions::summaries::{SessionSummaries, SessionSummaryCache};
use crate::app::sessions::{self, DetectedSettings, SessionStatus, Settings};
use crate::data::{accounts_tokens, app_users};
use crate::deployment::{Deployment, now};
use crate::services::agents::chat::ChatHistory;
use crate::services::agents::recent_writes::OpenFileCache;
use crate::services::apple_push::{ApplePushError, ApplePushSender};
use crate::services::terminal_app::ShowOutcome;

/// The server's data folder, beside its SQLite file (`axum/data/`): the
/// bells, the phones' push tokens and the APNs key.
const DATA_DIRECTORY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../axum/data");
/// How long a device's request for notifications waits for one.
const NOTIFICATIONS_WAIT: Duration = Duration::from_secs(25);
/// Apple's device tokens are 32 bytes in hex today, and may grow.
const APPLE_PUSH_TOKEN_LENGTHS: std::ops::RangeInclusive<usize> = 64..=200;

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
    /// Each device's bells, and the notifications it hasn't collected.
    notifications: SessionNotifications,
    /// Sends phones their notifications; `None` without an APNs key.
    apple_push: Option<ApplePushSender>,
}

impl Default for SessionHost {
    fn default() -> SessionHost {
        SessionHost::open(&PathBuf::from(DATA_DIRECTORY))
    }
}

impl SessionHost {
    /// The sessions' state, with the bells and the APNs key in `data_directory`.
    pub fn open(data_directory: &Path) -> SessionHost {
        let open_files = Arc::new(OpenFileCache::new());
        SessionHost {
            settings: OnceCell::new(),
            summaries: SessionSummaryCache::new(Arc::clone(&open_files)),
            open_files,
            artifacts: ArtifactsServer::from_environment(),
            notifications: open_notifications(data_directory),
            apple_push: open_apple_push(data_directory),
        }
    }

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

    /// The tmux sessions the app started, each with its agent status and
    /// `device_id`'s bell, and its worktrees without a session, as found up
    /// to a few seconds ago.
    pub async fn list(&self, device_id: &str) -> Result<SessionSummaries, SessionError> {
        let mut summaries = self.list_without_bells().await?;
        for session in &mut summaries.sessions {
            session.notifications_enabled = self.notifications.is_enabled(&session.name, device_id);
        }
        Ok(summaries)
    }

    /// The sessions list as cached, with every bell off.
    pub async fn list_without_bells(&self) -> Result<SessionSummaries, SessionError> {
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
        self.remove_bells_after(&closed, name);
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
        self.remove_bells_after(&merged, name);
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

    /// The bells and the notifications waiting, for the watcher.
    pub fn notifications(&self) -> &SessionNotifications {
        &self.notifications
    }

    /// Whether `device_id`'s bell is on for a session.
    pub async fn bell_get(&self, device_id: &str, name: &str) -> Result<bool, SessionError> {
        sessions::validate_app_session_name(self.settings().await, name).map_err(SessionError)?;
        Ok(self.notifications.is_enabled(name, device_id))
    }

    /// Turns `device_id`'s bell for a session on or off: while on, the
    /// device is notified each time the session's agent finishes.
    pub async fn bell_update(
        &self,
        device_id: &str,
        name: &str,
        enabled: bool,
    ) -> Result<bool, SessionError> {
        sessions::validate_app_session_name(self.settings().await, name).map_err(SessionError)?;
        self.notifications
            .set_enabled(name, device_id, enabled)
            .map_err(SessionError)?;
        Ok(enabled)
    }

    /// The notifications waiting for `device_id`, else the next ones within
    /// 25 seconds, else none: for the device to ask again at once.
    pub async fn notifications_wait(&self, device_id: &str) -> Vec<SessionNotification> {
        self.notifications
            .wait_for(device_id, NOTIFICATIONS_WAIT)
            .await
    }

    /// Keeps `token` as the Apple push token of the phone `device_id`.
    /// Phones send it at each launch, as it changes when the app is
    /// reinstalled.
    pub fn apple_push_token_update(
        &self,
        device_id: &str,
        token: &str,
    ) -> Result<(), SessionError> {
        if !APPLE_PUSH_TOKEN_LENGTHS.contains(&token.len())
            || !token.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(SessionError("That is not an Apple push token.".to_string()));
        }
        self.notifications
            .set_apple_push_token(device_id, token)
            .map_err(SessionError)
    }

    /// Pushes `notification` to each phone whose bell is on for its
    /// session, forgetting the tokens Apple says are no longer valid.
    pub async fn send_apple_push_notifications(&self, notification: &SessionNotification) {
        let Some(apple_push) = &self.apple_push else {
            return;
        };
        for (device_id, token) in self
            .notifications
            .list_apple_push_tokens_for(&notification.session_name)
        {
            match apple_push.send(&token, notification).await {
                Ok(()) => {}
                Err(error @ ApplePushError::DeviceTokenNotValid(_)) => {
                    tracing::warn!("Pushing to a phone: {error}; forgetting its token");
                    if let Err(error) = self
                        .notifications
                        .remove_apple_push_token(&device_id, &token)
                    {
                        tracing::warn!("Forgetting a phone's push token failed: {error}");
                    }
                }
                Err(error) => tracing::warn!("Pushing to a phone: {error}"),
            }
        }
    }

    /// Turns off every bell, and forgets the push token, of each device
    /// with a bell on for `name` whose sign-in no longer works: it signed
    /// out, or its account was deleted.
    pub async fn remove_signed_out_devices(&self, deployment: &Deployment, name: &str) {
        for device_id in self.notifications.list_device_ids_for(name) {
            match device_signs_in(deployment, &device_id).await {
                Ok(true) => {}
                Ok(false) => {
                    if let Err(error) = self.notifications.remove_device(&device_id) {
                        tracing::warn!("Forgetting a signed-out device failed: {error}");
                    }
                }
                Err(error) => tracing::warn!("Checking a device's sign-in failed: {error}"),
            }
        }
    }

    fn remove_bells_after(&self, finished: &Result<String, String>, name: &str) {
        if finished.is_ok()
            && let Err(error) = self.notifications.remove_sessions(&[name.to_string()])
        {
            tracing::warn!("Turning off the bells of {name} failed: {error}");
        }
    }
}

/// Whether the token whose hash is `device_id` still signs a live user in.
async fn device_signs_in(deployment: &Deployment, device_id: &str) -> Result<bool, sqlx::Error> {
    let (api_made_after, session_made_after) = accounts_tokens::made_after_to_be_valid(now());
    let mut tx = deployment.begin().await?;
    let user = app_users::authentication::get_by_token_hash(
        &mut tx,
        device_id,
        api_made_after,
        session_made_after,
    )
    .await?;
    tx.commit().await?;
    Ok(user.is_some_and(|user| user.deleted_at.is_none()))
}

fn open_notifications(data_directory: &Path) -> SessionNotifications {
    SessionNotifications::open(data_directory).unwrap_or_else(|error| {
        tracing::warn!("The bells could not be read, so they start empty: {error}");
        SessionNotifications::empty(data_directory)
    })
}

fn open_apple_push(data_directory: &Path) -> Option<ApplePushSender> {
    match ApplePushSender::open(data_directory) {
        Ok(Some(sender)) => {
            tracing::info!("Apple push notifications are on (sandbox)");
            Some(sender)
        }
        Ok(None) => None,
        Err(error) => {
            tracing::warn!("Apple push notifications are off: {error}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;
    use crate::deployment::Role;

    #[tokio::test]
    async fn session_host_turns_off_the_bells_of_a_device_that_signed_out() {
        let database = Database::open_sqlite_in_memory().await.expect("a database");
        let deployment = Deployment::new(database, Role::Server);
        let email = "dana@example.com";
        let password = "correct horse battery";
        crate::accounts::users::create(&deployment, email, password)
            .await
            .expect("an account");
        let phone = crate::accounts::sign_in_with_password(&deployment, email, password)
            .await
            .expect("signed in");
        let laptop = crate::accounts::sign_in_with_password(&deployment, email, password)
            .await
            .expect("signed in");
        let directory = tempfile::tempdir().expect("a directory");
        let host = SessionHost::open(directory.path());
        let phone_id = accounts_tokens::hash(&phone.token);
        let laptop_id = accounts_tokens::hash(&laptop.token);
        for device_id in [&phone_id, &laptop_id] {
            host.notifications()
                .set_enabled("cr-fix", device_id, true)
                .expect("a bell");
        }

        crate::accounts::sign_out(&deployment, &laptop.token)
            .await
            .expect("signed out");
        host.remove_signed_out_devices(&deployment, "cr-fix").await;

        assert_eq!(
            host.notifications().list_device_ids_for("cr-fix"),
            vec![phone_id]
        );
    }
}
