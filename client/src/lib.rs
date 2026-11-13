//! A typed client for the backend server's HTTP API: one method per
//! command, sending the request as JSON to `POST /api/<command>` with the
//! token, and reading the response or `{ "error": ... }` back.

use code_reviewer_core::analysis::file_diff::FileDiff;
use code_reviewer_core::analysis::function_diff::FunctionDiff;
use code_reviewer_core::analysis::service::{
    AnalyzeFileChangesRequest, AnalyzeFileChangesResponse, GetFileDiffsRequest,
    GetFunctionDiffsRequest, GetTestCallStackRequest, GetTestCallStackResponse, ListTestsRequest,
    ListTestsResponse,
};
use code_reviewer_core::git::{
    ListBranchesAndRecentCommitsRequest, ListBranchesAndRecentCommitsResponse,
};
use code_reviewer_server::requests::{
    CreatePairedDeviceRequest, CreateSessionRequest, PairedDeviceIdRequest,
    SendPromptToSessionRequest, SessionNameRequest, SetRemoteAccessEnabledRequest,
};
use code_reviewer_server::{
    ChatHistory, DetectedSettings, SessionList, SessionStatus, ShowOutcome,
};
use code_reviewer_server::{CreatedPairedDevice, PairingCode, RemoteAccessStatus};
use secrecy::{ExposeSecret, SecretString};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// Why a call failed. Displays as the server's own message when it sent one.
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    /// The command ran and failed, or the server refused the request.
    #[error("{message}")]
    Api { status: u16, message: String },
    /// The server couldn't be reached, or its answer couldn't be read.
    #[error("couldn't reach the backend: {0}")]
    Request(#[from] reqwest::Error),
    /// The server answered with an error that isn't the API's JSON.
    #[error("the backend answered {status}: {body}")]
    UnexpectedResponse { status: u16, body: String },
}

#[derive(Deserialize)]
struct ErrorBody {
    error: String,
}

/// For the commands that take no arguments.
#[derive(Serialize)]
struct EmptyBody {}

/// Calls the backend at `base_url` with `token`.
pub struct BackendClient {
    http: reqwest::Client,
    base_url: String,
    token: SecretString,
}

impl BackendClient {
    /// A client for the server at `base_url`, such as `http://127.0.0.1:52341`.
    pub fn new(base_url: String, token: SecretString) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url,
            token,
        }
    }

    /// A client for the server a pairing code points at, with its token.
    pub fn from_pairing_code(code: &PairingCode) -> Self {
        Self::new(code.url.clone(), code.token.clone())
    }

    /// The server this client calls.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    async fn post<Body, Response>(
        &self,
        command: &str,
        body: &Body,
    ) -> Result<Response, ClientError>
    where
        Body: Serialize + ?Sized,
        Response: DeserializeOwned,
    {
        let response = self
            .http
            .post(format!("{}/api/{command}", self.base_url))
            .bearer_auth(self.token.expose_secret())
            .json(body)
            .send()
            .await?;
        let status = response.status();
        if status.is_success() {
            return Ok(response.json().await?);
        }
        let text = response.text().await?;
        Err(match serde_json::from_str::<ErrorBody>(&text) {
            Ok(error_body) => ClientError::Api {
                status: status.as_u16(),
                message: error_body.error,
            },
            Err(_) => ClientError::UnexpectedResponse {
                status: status.as_u16(),
                body: text,
            },
        })
    }

    async fn post_session_name<Response: DeserializeOwned>(
        &self,
        command: &str,
        name: &str,
    ) -> Result<Response, ClientError> {
        let body = SessionNameRequest {
            name: name.to_string(),
        };
        self.post(command, &body).await
    }

    /* -- Reviews ---------------------------------------------------------- */

    /// The branches and latest commits of a repository, to choose what to review.
    pub async fn list_branches_and_recent_commits(
        &self,
        request: &ListBranchesAndRecentCommitsRequest,
    ) -> Result<ListBranchesAndRecentCommitsResponse, ClientError> {
        self.post("list_branches_and_recent_commits", request).await
    }

    /// What a comparison changed, grouped by file and by group.
    pub async fn analyze_file_changes(
        &self,
        request: &AnalyzeFileChangesRequest,
    ) -> Result<AnalyzeFileChangesResponse, ClientError> {
        self.post("analyze_file_changes", request).await
    }

    /// The whole of each changed function, with its added and removed lines marked.
    pub async fn get_function_diffs(
        &self,
        request: &GetFunctionDiffsRequest,
    ) -> Result<Vec<FunctionDiff>, ClientError> {
        self.post("get_function_diffs", request).await
    }

    /// The changed lines of files of any type, with a few lines of context.
    pub async fn get_file_diffs(
        &self,
        request: &GetFileDiffsRequest,
    ) -> Result<Vec<FileDiff>, ClientError> {
        self.post("get_file_diffs", request).await
    }

    /* -- Traces ----------------------------------------------------------- */

    /// Every test in the head of a comparison, marking the ones it added or modified.
    pub async fn list_tests(
        &self,
        request: &ListTestsRequest,
    ) -> Result<ListTestsResponse, ClientError> {
        self.post("list_tests", request).await
    }

    /// The calls below one test, walked top to bottom, each function marked against the base.
    pub async fn get_test_call_stack(
        &self,
        request: &GetTestCallStackRequest,
    ) -> Result<GetTestCallStackResponse, ClientError> {
        self.post("get_test_call_stack", request).await
    }

    /* -- Sessions --------------------------------------------------------- */

    /// The detected shell, terminal app and tmux, and where new sessions start.
    pub async fn get_session_settings(&self) -> Result<DetectedSettings, ClientError> {
        self.post("get_session_settings", &EmptyBody {}).await
    }

    /// The port a new session gets unless the user picks one.
    pub async fn get_default_session_port(&self) -> Result<u16, ClientError> {
        self.post("get_default_session_port", &EmptyBody {}).await
    }

    /// The tmux sessions the app started, each with its agent status, and
    /// its worktrees without a session.
    pub async fn list_sessions(&self) -> Result<SessionList, ClientError> {
        self.post("list_sessions", &EmptyBody {}).await
    }

    /// One of the app's tmux sessions, with its agent and git status.
    pub async fn get_session(&self, name: &str) -> Result<SessionStatus, ClientError> {
        self.post_session_name("get_session", name).await
    }

    /// The chat history of the agent in one of the app's sessions.
    pub async fn get_agent_chat_history(&self, name: &str) -> Result<ChatHistory, ClientError> {
        self.post_session_name("get_agent_chat_history", name).await
    }

    /// Types `prompt` into the agent's pane in one of the app's sessions and presses Enter.
    pub async fn send_prompt_to_session(
        &self,
        name: &str,
        prompt: &str,
    ) -> Result<(), ClientError> {
        let body = SendPromptToSessionRequest {
            name: name.to_string(),
            prompt: prompt.to_string(),
        };
        self.post("send_prompt_to_session", &body).await
    }

    /// Creates a worktree and a tmux session running `command` in it.
    /// Returns the session's full name, prefix included.
    pub async fn create_session(
        &self,
        request: &CreateSessionRequest,
    ) -> Result<String, ClientError> {
        self.post("create_session", request).await
    }

    /// Starts a tmux session again in one of the app's worktrees.
    pub async fn continue_session(&self, name: &str) -> Result<ShowOutcome, ClientError> {
        self.post_session_name("continue_session", name).await
    }

    /// Merges a session's branch into the main branch and removes the session.
    /// Returns what was done.
    pub async fn merge_session(&self, name: &str) -> Result<String, ClientError> {
        self.post_session_name("merge_session", name).await
    }

    /// Removes a session's worktree, keeping its branch, and closes its tmux
    /// session. Returns what was done.
    pub async fn close_session(&self, name: &str) -> Result<String, ClientError> {
        self.post_session_name("close_session", name).await
    }

    /// Brings the terminal window of one of the app's sessions to the front, or opens one.
    pub async fn show_session_in_terminal_app(
        &self,
        name: &str,
    ) -> Result<ShowOutcome, ClientError> {
        self.post_session_name("show_session_in_terminal_app", name)
            .await
    }

    /// A session's `http://localhost:<port>`, to open in a browser.
    pub async fn get_session_browser_url(&self, name: &str) -> Result<String, ClientError> {
        self.post_session_name("get_session_browser_url", name)
            .await
    }

    /// The URL on the server's computer of the artifacts of a session's
    /// checkout, to open in a browser.
    pub async fn get_session_artifacts_url(&self, name: &str) -> Result<String, ClientError> {
        self.post_session_name("get_session_artifacts_url", name)
            .await
    }

    /* -- Remote access (the localhost listener only) ------------------------ */

    /// Whether remote access is on, its URL, and the paired devices.
    pub async fn get_remote_access_status(&self) -> Result<RemoteAccessStatus, ClientError> {
        self.post("get_remote_access_status", &EmptyBody {}).await
    }

    /// Turns remote access on or off.
    pub async fn set_remote_access_enabled(
        &self,
        enabled: bool,
    ) -> Result<RemoteAccessStatus, ClientError> {
        let body = SetRemoteAccessEnabledRequest { enabled };
        self.post("set_remote_access_enabled", &body).await
    }

    /// Pairs a device, returning it with its pairing code.
    pub async fn create_paired_device(
        &self,
        name: &str,
    ) -> Result<CreatedPairedDevice, ClientError> {
        let body = CreatePairedDeviceRequest {
            name: name.to_string(),
        };
        self.post("create_paired_device", &body).await
    }

    /// Unpairs a device.
    pub async fn revoke_paired_device(&self, id: &str) -> Result<RemoteAccessStatus, ClientError> {
        let body = PairedDeviceIdRequest { id: id.to_string() };
        self.post("revoke_paired_device", &body).await
    }
}
