//! The backend's commands, one method each, over the state they share.

use std::sync::Arc;

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

use crate::agent_chat::ChatHistory;
use crate::analysis::run_analysis;
use crate::artifacts::ArtifactsServer;
use crate::sessions::{self, DetectedSettings, SessionList, SessionStatus, Settings};
use crate::terminal_app::ShowOutcome;

/// What the commands share. One instance lives for the whole process.
pub struct Backend {
    /// Holds the project index cache.
    analysis: Arc<AnalysisService>,
    /// The shell, terminal app and tmux, detected on first use.
    session_settings: OnceCell<DetectedSettings>,
    /// Where bin/dev keeps and serves artifacts; `None` when it didn't start the app.
    artifacts: Option<ArtifactsServer>,
}

impl Default for Backend {
    fn default() -> Self {
        Self::new()
    }
}

impl Backend {
    /// A backend with an empty analysis cache, session settings not yet
    /// detected, and the artifacts bin/dev set up, if it started the app.
    pub fn new() -> Self {
        Self {
            analysis: Arc::new(AnalysisService::new()),
            session_settings: OnceCell::new(),
            artifacts: ArtifactsServer::from_environment(),
        }
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

    /// The tmux sessions the app started, each with its agent status, and
    /// its worktrees without a session.
    pub async fn list_sessions(&self) -> Result<SessionList, String> {
        sessions::list_sessions(self.settings().await).await
    }

    /// One of the app's tmux sessions, with its agent and git status. `name`
    /// is the full session name, prefix included.
    pub async fn get_session(&self, name: &str) -> Result<SessionStatus, String> {
        sessions::get_session(self.settings().await, name).await
    }

    /// The chat history of the agent in one of the app's sessions: its
    /// messages and tool calls, read from the transcript the agent writes.
    pub async fn get_agent_chat_history(&self, name: &str) -> Result<ChatHistory, String> {
        sessions::get_agent_chat_history(self.settings().await, name).await
    }

    /// Types `prompt` into the agent's pane in one of the app's sessions and
    /// presses Enter, as if the user typed it there.
    pub async fn send_prompt_to_session(&self, name: &str, prompt: &str) -> Result<(), String> {
        sessions::send_prompt_to_session(self.settings().await, name, prompt).await
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

    /// Creates a git worktree of `repository` on a branch named `name` and a
    /// tmux session named `<prefix>-<name>` in it with `PORT` set to `port`,
    /// types `command` into it and opens it in the terminal app. Returns the
    /// session's full name, prefix included.
    pub async fn create_session(
        &self,
        name: &str,
        repository: &str,
        command: &str,
        port: u16,
    ) -> Result<String, String> {
        sessions::create_session(self.settings().await, name, repository, command, port).await
    }

    /// Starts a tmux session again in one of the app's worktrees, named after
    /// it, and opens it in the terminal app. `name` is the worktree's name,
    /// prefix included.
    pub async fn continue_session(&self, name: &str) -> Result<ShowOutcome, String> {
        sessions::continue_session(self.settings().await, name).await
    }

    /// Merges a session's branch into the main branch (fast-forward only),
    /// removes its worktree, deletes the branch and closes its tmux session.
    /// `name` is the worktree's name, prefix included. Returns what was done.
    pub async fn merge_session(&self, name: &str) -> Result<String, String> {
        sessions::merge_session(self.settings().await, self.artifacts.as_ref(), name).await
    }

    /// Removes a session's worktree, keeping its branch, and closes its tmux
    /// session. `name` is the worktree's name, prefix included. Returns what
    /// was done.
    pub async fn close_session(&self, name: &str) -> Result<String, String> {
        sessions::close_session(self.settings().await, self.artifacts.as_ref(), name).await
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
}
