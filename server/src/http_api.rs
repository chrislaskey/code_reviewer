//! The backend's HTTP API: one `POST /api/<command>` route per command,
//! each one call to a [`Backend`] method, with JSON in and out. A failure is
//! a status code and `{ "error": "..." }`. The routers know nothing of how
//! requests reach them; each listener serves the one for its clients.

use std::sync::Arc;

use axum::extract::rejection::JsonRejection;
use axum::extract::{FromRequest, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
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
use serde::Serialize;

use crate::agent_chat::ChatHistory;
use crate::backend::Backend;
use crate::remote_access::{CreatedPairedDevice, RemoteAccess, RemoteAccessStatus};
use crate::requests::{
    CreatePairedDeviceRequest, CreateSessionRequest, PairedDeviceIdRequest,
    SendPromptToSessionRequest, SessionNameRequest, SetRemoteAccessEnabledRequest,
};
use crate::sessions::{DetectedSettings, SessionList, SessionStatus};
use crate::terminal_app::ShowOutcome;

/// Every route, for the desktop app on this computer: the commands any
/// client may run, the ones that act on this computer's screen, and the
/// management of remote access.
pub fn localhost_router(backend: Arc<Backend>, remote_access: Arc<RemoteAccess>) -> Router {
    Router::new()
        .merge(routes_for_every_client().with_state(Arc::clone(&backend)))
        .merge(routes_for_this_computer().with_state(backend))
        .merge(routes_for_remote_access().with_state(remote_access))
        .fallback(route_not_found)
        .method_not_allowed_fallback(method_not_allowed)
}

/// The routes for paired devices: not the ones that act on this computer's
/// screen, nor remote access, so a device can't pair others.
pub fn tailnet_router(backend: Arc<Backend>) -> Router {
    Router::new()
        .merge(routes_for_every_client().with_state(backend))
        .fallback(route_not_found)
        .method_not_allowed_fallback(method_not_allowed)
}

fn routes_for_every_client() -> Router<Arc<Backend>> {
    Router::new()
        .route(
            "/api/list_branches_and_recent_commits",
            post(list_branches_and_recent_commits),
        )
        .route("/api/analyze_file_changes", post(analyze_file_changes))
        .route("/api/get_function_diffs", post(get_function_diffs))
        .route("/api/get_file_diffs", post(get_file_diffs))
        .route("/api/list_tests", post(list_tests))
        .route("/api/get_test_call_stack", post(get_test_call_stack))
        .route("/api/get_session_settings", post(get_session_settings))
        .route(
            "/api/get_default_session_port",
            post(get_default_session_port),
        )
        .route("/api/list_sessions", post(list_sessions))
        .route("/api/get_session", post(get_session))
        .route("/api/get_agent_chat_history", post(get_agent_chat_history))
        .route("/api/send_prompt_to_session", post(send_prompt_to_session))
        .route("/api/create_session", post(create_session))
        .route("/api/continue_session", post(continue_session))
        .route("/api/merge_session", post(merge_session))
        .route("/api/close_session", post(close_session))
}

fn routes_for_this_computer() -> Router<Arc<Backend>> {
    Router::new()
        .route(
            "/api/show_session_in_terminal_app",
            post(show_session_in_terminal_app),
        )
        .route(
            "/api/open_session_in_browser",
            post(open_session_in_browser),
        )
}

fn routes_for_remote_access() -> Router<Arc<RemoteAccess>> {
    Router::new()
        .route(
            "/api/get_remote_access_status",
            post(get_remote_access_status),
        )
        .route(
            "/api/set_remote_access_enabled",
            post(set_remote_access_enabled),
        )
        .route("/api/create_paired_device", post(create_paired_device))
        .route("/api/revoke_paired_device", post(revoke_paired_device))
}

/// An API failure: the status code and the message sent as `{ "error": ... }`.
#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    pub(crate) fn new(status: StatusCode, message: &str) -> Self {
        Self {
            status,
            message: message.to_string(),
        }
    }
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = ErrorBody {
            error: self.message,
        };
        (self.status, Json(body)).into_response()
    }
}

/// A command that ran and failed. The backend's errors are plain messages,
/// so they can't be told apart into finer status codes.
impl From<String> for ApiError {
    fn from(message: String) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message,
        }
    }
}

/// A body that isn't the JSON the route expects.
impl From<JsonRejection> for ApiError {
    fn from(rejection: JsonRejection) -> Self {
        Self {
            status: rejection.status(),
            message: rejection.body_text(),
        }
    }
}

/// Axum's `Json` extractor, failing with an [`ApiError`].
#[derive(FromRequest)]
#[from_request(via(Json), rejection(ApiError))]
struct JsonBody<T>(T);

type ApiResult<T> = Result<Json<T>, ApiError>;
type BackendState = State<Arc<Backend>>;

async fn route_not_found() -> ApiError {
    ApiError::new(StatusCode::NOT_FOUND, "no such API route")
}

async fn method_not_allowed() -> ApiError {
    ApiError::new(StatusCode::METHOD_NOT_ALLOWED, "API routes take POST")
}

/* -- Reviews -------------------------------------------------------------- */

async fn list_branches_and_recent_commits(
    State(backend): BackendState,
    JsonBody(request): JsonBody<ListBranchesAndRecentCommitsRequest>,
) -> ApiResult<ListBranchesAndRecentCommitsResponse> {
    Ok(Json(
        backend.list_branches_and_recent_commits(request).await?,
    ))
}

async fn analyze_file_changes(
    State(backend): BackendState,
    JsonBody(request): JsonBody<AnalyzeFileChangesRequest>,
) -> ApiResult<AnalyzeFileChangesResponse> {
    Ok(Json(backend.analyze_file_changes(request).await?))
}

async fn get_function_diffs(
    State(backend): BackendState,
    JsonBody(request): JsonBody<GetFunctionDiffsRequest>,
) -> ApiResult<Vec<FunctionDiff>> {
    Ok(Json(backend.get_function_diffs(request).await?))
}

async fn get_file_diffs(
    State(backend): BackendState,
    JsonBody(request): JsonBody<GetFileDiffsRequest>,
) -> ApiResult<Vec<FileDiff>> {
    Ok(Json(backend.get_file_diffs(request).await?))
}

/* -- Traces --------------------------------------------------------------- */

async fn list_tests(
    State(backend): BackendState,
    JsonBody(request): JsonBody<ListTestsRequest>,
) -> ApiResult<ListTestsResponse> {
    Ok(Json(backend.list_tests(request).await?))
}

async fn get_test_call_stack(
    State(backend): BackendState,
    JsonBody(request): JsonBody<GetTestCallStackRequest>,
) -> ApiResult<GetTestCallStackResponse> {
    Ok(Json(backend.get_test_call_stack(request).await?))
}

/* -- Sessions ------------------------------------------------------------- */

async fn get_session_settings(State(backend): BackendState) -> ApiResult<DetectedSettings> {
    Ok(Json(backend.get_session_settings().await?))
}

async fn get_default_session_port(State(backend): BackendState) -> ApiResult<u16> {
    Ok(Json(backend.get_default_session_port().await?))
}

async fn list_sessions(State(backend): BackendState) -> ApiResult<SessionList> {
    Ok(Json(backend.list_sessions().await?))
}

async fn get_session(
    State(backend): BackendState,
    JsonBody(request): JsonBody<SessionNameRequest>,
) -> ApiResult<SessionStatus> {
    Ok(Json(backend.get_session(&request.name).await?))
}

async fn get_agent_chat_history(
    State(backend): BackendState,
    JsonBody(request): JsonBody<SessionNameRequest>,
) -> ApiResult<ChatHistory> {
    Ok(Json(backend.get_agent_chat_history(&request.name).await?))
}

async fn send_prompt_to_session(
    State(backend): BackendState,
    JsonBody(request): JsonBody<SendPromptToSessionRequest>,
) -> ApiResult<()> {
    Ok(Json(
        backend
            .send_prompt_to_session(&request.name, &request.prompt)
            .await?,
    ))
}

async fn create_session(
    State(backend): BackendState,
    JsonBody(request): JsonBody<CreateSessionRequest>,
) -> ApiResult<String> {
    let CreateSessionRequest {
        name,
        repository,
        command,
        port,
    } = request;
    Ok(Json(
        backend
            .create_session(&name, &repository, &command, port)
            .await?,
    ))
}

async fn continue_session(
    State(backend): BackendState,
    JsonBody(request): JsonBody<SessionNameRequest>,
) -> ApiResult<ShowOutcome> {
    Ok(Json(backend.continue_session(&request.name).await?))
}

async fn merge_session(
    State(backend): BackendState,
    JsonBody(request): JsonBody<SessionNameRequest>,
) -> ApiResult<String> {
    Ok(Json(backend.merge_session(&request.name).await?))
}

async fn close_session(
    State(backend): BackendState,
    JsonBody(request): JsonBody<SessionNameRequest>,
) -> ApiResult<String> {
    Ok(Json(backend.close_session(&request.name).await?))
}

async fn show_session_in_terminal_app(
    State(backend): BackendState,
    JsonBody(request): JsonBody<SessionNameRequest>,
) -> ApiResult<ShowOutcome> {
    Ok(Json(
        backend.show_session_in_terminal_app(&request.name).await?,
    ))
}

async fn open_session_in_browser(
    State(backend): BackendState,
    JsonBody(request): JsonBody<SessionNameRequest>,
) -> ApiResult<String> {
    Ok(Json(backend.open_session_in_browser(&request.name).await?))
}

/* -- Remote access (localhost only) --------------------------------------- */

type RemoteAccessState = State<Arc<RemoteAccess>>;

async fn get_remote_access_status(
    State(remote_access): RemoteAccessState,
) -> ApiResult<RemoteAccessStatus> {
    Ok(Json(remote_access.get_status().await))
}

async fn set_remote_access_enabled(
    State(remote_access): RemoteAccessState,
    JsonBody(request): JsonBody<SetRemoteAccessEnabledRequest>,
) -> ApiResult<RemoteAccessStatus> {
    Ok(Json(remote_access.set_enabled(request.enabled).await?))
}

async fn create_paired_device(
    State(remote_access): RemoteAccessState,
    JsonBody(request): JsonBody<CreatePairedDeviceRequest>,
) -> ApiResult<CreatedPairedDevice> {
    Ok(Json(
        remote_access.create_paired_device(&request.name).await?,
    ))
}

async fn revoke_paired_device(
    State(remote_access): RemoteAccessState,
    JsonBody(request): JsonBody<PairedDeviceIdRequest>,
) -> ApiResult<RemoteAccessStatus> {
    Ok(Json(remote_access.revoke_paired_device(&request.id).await?))
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::process::Command;

    use axum::body::{Body, to_bytes};
    use axum::http::{Method, Request};
    use serde_json::{Value, json};
    use tower::ServiceExt;

    use super::*;

    async fn send(method: Method, path: &str, body: &str) -> (StatusCode, Value) {
        let request = Request::builder()
            .method(method)
            .uri(path)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();
        let data_directory = tempfile::tempdir().unwrap();
        let response = test_localhost_router(data_directory.path())
            .oneshot(request)
            .await
            .unwrap();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        (status, serde_json::from_slice(&bytes).unwrap())
    }

    fn test_localhost_router(data_directory: &Path) -> Router {
        let backend = Arc::new(Backend::new());
        let remote_access =
            Arc::new(RemoteAccess::open(data_directory, Arc::clone(&backend)).unwrap());
        localhost_router(backend, remote_access)
    }

    async fn send_to(app: Router, path: &str, body: &str) -> StatusCode {
        let request = Request::builder()
            .method(Method::POST)
            .uri(path)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();
        app.oneshot(request).await.unwrap().status()
    }

    fn git(directory: &Path, arguments: &[&str]) {
        let status = Command::new("git")
            .args(["-c", "user.name=Test", "-c", "user.email=test@example.com"])
            .args(arguments)
            .current_dir(directory)
            .status()
            .unwrap();
        assert!(status.success(), "git {arguments:?} failed");
    }

    fn repository_with_one_commit() -> tempfile::TempDir {
        let directory = tempfile::tempdir().unwrap();
        git(
            directory.path(),
            &["init", "--quiet", "--initial-branch=main"],
        );
        std::fs::write(directory.path().join("README.md"), "hello\n").unwrap();
        git(directory.path(), &["add", "."]);
        git(
            directory.path(),
            &["commit", "--quiet", "-m", "First commit"],
        );
        directory
    }

    #[tokio::test]
    async fn a_command_answers_with_its_response_as_json() {
        let repository = repository_with_one_commit();
        let body = json!({ "repo_path": repository.path() }).to_string();

        let (status, response) =
            send(Method::POST, "/api/list_branches_and_recent_commits", &body).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(response["current_branch"], "main");
        assert_eq!(response["recent_commits"][0]["subject"], "First commit");
    }

    #[tokio::test]
    async fn a_failed_command_is_a_server_error_with_its_message() {
        let body = json!({ "repo_path": "/no/such/repository" }).to_string();

        let (status, response) =
            send(Method::POST, "/api/list_branches_and_recent_commits", &body).await;

        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert!(!response["error"].as_str().unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_body_that_is_not_json_is_a_bad_request() {
        let (status, response) =
            send(Method::POST, "/api/list_branches_and_recent_commits", "{").await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(response["error"].is_string());
    }

    #[tokio::test]
    async fn a_body_missing_a_field_is_unprocessable() {
        let (status, response) =
            send(Method::POST, "/api/list_branches_and_recent_commits", "{}").await;

        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert!(response["error"].as_str().unwrap().contains("repo_path"));
    }

    #[tokio::test]
    async fn an_unknown_route_is_not_found() {
        let (status, response) = send(Method::POST, "/api/no_such_command", "{}").await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(response["error"], "no such API route");
    }

    #[tokio::test]
    async fn a_route_only_takes_post() {
        let (status, response) = send(Method::GET, "/api/list_sessions", "").await;

        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(response["error"], "API routes take POST");
    }

    #[tokio::test]
    async fn the_tailnet_router_leaves_out_this_computers_screen_and_remote_access() {
        let data_directory = tempfile::tempdir().unwrap();
        let body = json!({ "name": "cr-x", "enabled": true, "id": "x" }).to_string();

        for path in [
            "/api/show_session_in_terminal_app",
            "/api/open_session_in_browser",
            "/api/get_remote_access_status",
            "/api/set_remote_access_enabled",
            "/api/create_paired_device",
            "/api/revoke_paired_device",
        ] {
            let tailnet_status =
                send_to(tailnet_router(Arc::new(Backend::new())), path, &body).await;
            assert_eq!(
                tailnet_status,
                StatusCode::NOT_FOUND,
                "{path} on the tailnet"
            );
        }
        let localhost_status = send_to(
            test_localhost_router(data_directory.path()),
            "/api/get_remote_access_status",
            "{}",
        )
        .await;
        assert_eq!(localhost_status, StatusCode::OK);
    }
}
