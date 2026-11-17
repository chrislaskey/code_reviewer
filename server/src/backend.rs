//! The backend's commands, one method each, over the state they share.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use code_reviewer_core::analysis::file_diff::FileDiff;
use code_reviewer_core::analysis::function_diff::FunctionDiff;
use code_reviewer_core::analysis::service::{
    AnalysisService, AnalyzeFileChangesRequest, AnalyzeFileChangesResponse, GetFileDiffsRequest,
    GetFunctionDiffsRequest, GetTestCallStackRequest, GetTestCallStackResponse, ListTestsRequest,
    ListTestsResponse,
};
use code_reviewer_core::git::{
    self as core_git, ListBranchesAndRecentCommitsRequest, ListBranchesAndRecentCommitsResponse,
};
use tokio::sync::OnceCell;

use crate::access::DESKTOP_DEVICE_ID;
use crate::agent_chat::ChatHistory;
use crate::analysis::run_analysis;
use crate::apple_push::{ApplePushError, ApplePushSender};
use crate::artifacts::ArtifactsServer;
use crate::recent_writes::OpenFileCache;
use crate::session_notifications::{SessionNotification, SessionNotifications};
use crate::session_summaries::{SessionSummaries, SessionSummaryCache};
use crate::sessions::{self, DetectedSettings, SessionStatus, Settings};
use crate::terminal_app::ShowOutcome;

/// How long a device's request for notifications waits for one.
const SESSION_NOTIFICATIONS_WAIT: Duration = Duration::from_secs(25);
/// Apple's device tokens are 32 bytes in hex today, and may grow.
const APPLE_PUSH_TOKEN_LENGTHS: std::ops::RangeInclusive<usize> = 64..=200;

/// What the commands share. One instance lives for the whole process.
pub struct Backend {
    /// Holds the project index cache.
    analysis: Arc<AnalysisService>,
    /// The shell, terminal app and tmux, detected on first use.
    session_settings: OnceCell<DetectedSettings>,
    /// The sessions page's list, served at once and refreshed behind it.
    session_summaries: SessionSummaryCache,
    /// The file each Cursor or Codex process writes, so `lsof` isn't run on
    /// every refresh.
    open_files: Arc<OpenFileCache>,
    /// Where bin/dev keeps and serves artifacts; `None` when it didn't start the app.
    artifacts: Option<ArtifactsServer>,
    /// Each device's bells, and the notifications it hasn't collected.
    session_notifications: SessionNotifications,
    /// Sends phones their notifications; `None` without an APNs key.
    apple_push: Option<ApplePushSender>,
}

impl Backend {
    /// A backend with an empty analysis cache, session settings not yet
    /// detected, the artifacts bin/dev set up, if it started the app, and
    /// the bells kept in `data_directory`.
    pub fn new(data_directory: &Path) -> Result<Self, String> {
        let open_files = Arc::new(OpenFileCache::new());
        let session_notifications = SessionNotifications::open(data_directory)?;
        Ok(Self {
            analysis: Arc::new(AnalysisService::new()),
            session_settings: OnceCell::new(),
            session_summaries: SessionSummaryCache::new(Arc::clone(&open_files)),
            open_files,
            artifacts: ArtifactsServer::from_environment(),
            session_notifications,
            apple_push: open_apple_push(data_directory),
        })
    }

    pub(crate) fn session_notifications(&self) -> &SessionNotifications {
        &self.session_notifications
    }

    async fn detected_settings(&self) -> &DetectedSettings {
        self.session_settings
            .get_or_init(sessions::detect_settings)
            .await
    }

    async fn settings(&self) -> &Settings {
        &self.detected_settings().await.settings
    }

    /* -- Reviews ---------------------------------------------------------- */

    /// The branches and latest commits of a repository, to choose what to review.
    pub async fn list_branches_and_recent_commits(
        &self,
        request: ListBranchesAndRecentCommitsRequest,
    ) -> Result<ListBranchesAndRecentCommitsResponse, String> {
        tokio::task::spawn_blocking(move || core_git::list_branches_and_recent_commits(request))
            .await
            .map_err(|error| error.to_string())?
            .map_err(|error| error.to_string())
    }

    /// What a comparison changed, grouped by file and by group.
    pub async fn analyze_file_changes(
        &self,
        request: AnalyzeFileChangesRequest,
    ) -> Result<AnalyzeFileChangesResponse, String> {
        run_analysis(&self.analysis, move |service| {
            service.analyze_file_changes(request)
        })
        .await
    }

    /// The whole of each changed function, with its added and removed lines marked.
    pub async fn get_function_diffs(
        &self,
        request: GetFunctionDiffsRequest,
    ) -> Result<Vec<FunctionDiff>, String> {
        run_analysis(&self.analysis, move |service| {
            service.get_function_diffs(request)
        })
        .await
    }

    /// The changed lines of files of any type, with a few lines of context.
    pub async fn get_file_diffs(
        &self,
        request: GetFileDiffsRequest,
    ) -> Result<Vec<FileDiff>, String> {
        run_analysis(&self.analysis, move |service| {
            service.get_file_diffs(request)
        })
        .await
    }

    /* -- Traces ----------------------------------------------------------- */

    /// Every test in the head of a comparison, marking the ones it added or modified.
    pub async fn list_tests(&self, request: ListTestsRequest) -> Result<ListTestsResponse, String> {
        run_analysis(&self.analysis, move |service| service.list_tests(request)).await
    }

    /// The calls below one test, walked top to bottom, each project function
    /// shown in full with its lines marked against the base.
    pub async fn get_test_call_stack(
        &self,
        request: GetTestCallStackRequest,
    ) -> Result<GetTestCallStackResponse, String> {
        run_analysis(&self.analysis, move |service| {
            service.get_test_call_stack(request)
        })
        .await
    }

    /* -- Sessions --------------------------------------------------------- */

    /// The detected shell, terminal app and tmux, and where new sessions start.
    pub async fn get_session_settings(&self) -> Result<DetectedSettings, String> {
        Ok(self.detected_settings().await.clone())
    }

    /// The port a new session gets unless the user picks one: the lowest free
    /// one among the app's running sessions.
    pub async fn get_default_session_port(&self) -> Result<u16, String> {
        sessions::get_default_session_port(self.settings().await).await
    }

    /// The tmux sessions the app started, each with its agent status and
    /// `device_id`'s bell, and its worktrees without a session, as found up
    /// to a few seconds ago.
    pub async fn list_session_summaries(
        &self,
        device_id: &str,
    ) -> Result<SessionSummaries, String> {
        let mut summaries = self.list_cached_session_summaries().await?;
        for session in &mut summaries.sessions {
            session.notifications_enabled = self
                .session_notifications
                .is_enabled(&session.name, device_id);
        }
        Ok(summaries)
    }

    /// The sessions list as cached, with every bell off.
    pub(crate) async fn list_cached_session_summaries(&self) -> Result<SessionSummaries, String> {
        self.session_summaries
            .list_session_summaries(self.settings().await)
            .await
    }

    /// One of the app's tmux sessions, with its agent and git status. `name`
    /// is the full session name, prefix included.
    pub async fn get_session(&self, name: &str) -> Result<SessionStatus, String> {
        sessions::get_session(self.settings().await, &self.open_files, name).await
    }

    /// The chat history of the agent in one of the app's sessions: its
    /// messages and tool calls, read from the transcript the agent writes.
    pub async fn get_agent_chat_history(&self, name: &str) -> Result<ChatHistory, String> {
        sessions::get_agent_chat_history(self.settings().await, &self.open_files, name).await
    }

    /// Types `prompt` into the agent's pane in one of the app's sessions and
    /// presses Enter, as if the user typed it there.
    pub async fn send_prompt_to_session(&self, name: &str, prompt: &str) -> Result<(), String> {
        sessions::send_prompt_to_session(self.settings().await, &self.open_files, name, prompt)
            .await
    }

    /// Saves an image, sent as base64, in the artifacts of a session's
    /// checkout and returns its path, to put in a prompt for its agent.
    pub async fn save_prompt_image_for_session(
        &self,
        name: &str,
        media_type: &str,
        data_base64: &str,
    ) -> Result<String, String> {
        sessions::save_prompt_image_for_session(
            self.settings().await,
            self.artifacts.as_ref(),
            name,
            media_type,
            data_base64,
        )
        .await
    }

    /// Creates a git worktree of `repository` on a branch named `name`, from
    /// `base_branch` unless the branch exists, and a tmux session named `<prefix>-<name>` in it with `PORT` set to `port`,
    /// types `command` into it and opens it in the terminal app. Returns the
    /// session's full name, prefix included.
    pub async fn create_session(
        &self,
        name: &str,
        repository: &str,
        base_branch: &str,
        command: &str,
        port: u16,
    ) -> Result<String, String> {
        let created = sessions::create_session(
            self.settings().await,
            name,
            repository,
            base_branch,
            command,
            port,
        )
        .await;
        self.session_summaries.forget().await;
        created
    }

    /// Starts a tmux session again in one of the app's worktrees, named after
    /// it, and opens it in the terminal app. `name` is the worktree's name,
    /// prefix included.
    pub async fn continue_session(&self, name: &str) -> Result<ShowOutcome, String> {
        let continued = sessions::continue_session(self.settings().await, name).await;
        self.session_summaries.forget().await;
        continued
    }

    /// Merges a session's branch into `into_branch`, the main branch or its
    /// base branch (fast-forward only), removes its worktree, deletes the
    /// branch and closes its tmux session. `name` is the worktree's name,
    /// prefix included. Returns what was done.
    pub async fn merge_session(&self, name: &str, into_branch: &str) -> Result<String, String> {
        let merged = sessions::merge_session(
            self.settings().await,
            self.artifacts.as_ref(),
            name,
            into_branch,
        )
        .await;
        self.session_summaries.forget().await;
        self.remove_session_notifications_after(&merged, name);
        merged
    }

    /// Removes a session's worktree, keeping its branch, and closes its tmux
    /// session. `name` is the worktree's name, prefix included. Returns what
    /// was done.
    pub async fn close_session(&self, name: &str) -> Result<String, String> {
        let closed =
            sessions::close_session(self.settings().await, self.artifacts.as_ref(), name).await;
        self.session_summaries.forget().await;
        self.remove_session_notifications_after(&closed, name);
        closed
    }

    fn remove_session_notifications_after(&self, finished: &Result<String, String>, name: &str) {
        if finished.is_ok()
            && let Err(error) = self
                .session_notifications
                .remove_sessions(&[name.to_string()])
        {
            log::warn!("Turning off the bells of {name} failed: {error}");
        }
    }

    /// Brings the terminal window of one of the app's sessions to the front,
    /// or opens one. `name` is the full session name, prefix included.
    pub async fn show_session_in_terminal_app(&self, name: &str) -> Result<ShowOutcome, String> {
        sessions::show_session_in_terminal_app(self.settings().await, name).await
    }

    /// A session's `http://localhost:<port>`, to open in a browser.
    pub async fn get_session_browser_url(&self, name: &str) -> Result<String, String> {
        sessions::get_session_browser_url(self.settings().await, name).await
    }

    /// The URL on this computer of the artifacts of a session's checkout,
    /// to open in a browser. An error when bin/dev didn't start the app.
    pub async fn get_session_artifacts_url(&self, name: &str) -> Result<String, String> {
        sessions::get_session_artifacts_url(self.settings().await, self.artifacts.as_ref(), name)
            .await
    }

    /* -- Session notifications -------------------------------------------- */

    /// Whether `device_id`'s bell is on for one of the app's sessions.
    pub async fn get_session_notifications_enabled(
        &self,
        device_id: &str,
        name: &str,
    ) -> Result<bool, String> {
        sessions::validate_app_session_name(self.settings().await, name)?;
        Ok(self.session_notifications.is_enabled(name, device_id))
    }

    /// Turns `device_id`'s bell for one of the app's sessions on or off: while
    /// it's on, the device is notified each time the session's agent
    /// finishes. Returns whether it's on.
    pub async fn set_session_notifications_enabled(
        &self,
        device_id: &str,
        name: &str,
        enabled: bool,
    ) -> Result<bool, String> {
        sessions::validate_app_session_name(self.settings().await, name)?;
        self.session_notifications
            .set_enabled(name, device_id, enabled)?;
        Ok(enabled)
    }

    /// Keeps `token` as the Apple push token of the paired phone `device_id`,
    /// for it to be notified while suspended. Phones send it at each launch,
    /// since it changes when the app is reinstalled.
    pub fn set_apple_push_token(&self, device_id: &str, token: &str) -> Result<(), String> {
        if device_id == DESKTOP_DEVICE_ID {
            return Err("Only a paired phone has a push token.".to_string());
        }
        if !APPLE_PUSH_TOKEN_LENGTHS.contains(&token.len())
            || !token.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err("That is not an Apple push token.".to_string());
        }
        self.session_notifications
            .set_apple_push_token(device_id, token)
    }

    /// Pushes `notification` to each phone whose bell is on for its session,
    /// forgetting the tokens Apple says are no longer valid.
    pub(crate) async fn send_apple_push_notifications(&self, notification: &SessionNotification) {
        let Some(apple_push) = &self.apple_push else {
            return;
        };
        for (device_id, token) in self
            .session_notifications
            .list_apple_push_tokens_for(&notification.session_name)
        {
            match apple_push.send(&token, notification).await {
                Ok(()) => {}
                Err(error @ ApplePushError::DeviceTokenNotValid(_)) => {
                    log::warn!("Pushing to {device_id}: {error}; forgetting its token");
                    if let Err(error) = self
                        .session_notifications
                        .remove_apple_push_token(&device_id, &token)
                    {
                        log::warn!("Forgetting {device_id}'s push token failed: {error}");
                    }
                }
                Err(error) => log::warn!("Pushing to {device_id}: {error}"),
            }
        }
    }

    /// The notifications waiting for `device_id`, else the next ones within
    /// 25 seconds, else none: for the device to ask again at once.
    pub async fn wait_for_session_notifications(
        &self,
        device_id: &str,
    ) -> Result<Vec<SessionNotification>, String> {
        Ok(self
            .session_notifications
            .wait_for(device_id, SESSION_NOTIFICATIONS_WAIT)
            .await)
    }
}

/// The APNs sender, or `None` when push isn't set up or its files are broken.
fn open_apple_push(data_directory: &Path) -> Option<ApplePushSender> {
    match ApplePushSender::open(data_directory) {
        Ok(Some(sender)) => {
            log::info!("Apple push notifications are on (sandbox)");
            Some(sender)
        }
        Ok(None) => None,
        Err(error) => {
            log::warn!("Apple push notifications are off: {error}");
            None
        }
    }
}
