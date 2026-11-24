//! The artifacts at `/artifacts/`, see `artifacts/README.md`: open to
//! whoever reaches the server, as links in a browser carry no token. Each
//! is sent sandboxed, so an artifact's scripts run on an origin of their
//! own, never as the signed-in pages.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, Request, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use tower::ServiceExt;
use tower_http::services::ServeFile;

use app_core::app::server_state::ServerState;
use app_core::app::sessions::artifacts::FoundArtifact;

/// Scripts run, on an opaque origin with no cookie of the server's.
const SANDBOX_POLICY: &str = "sandbox allow-scripts allow-popups allow-downloads";

/// `/artifacts/`, the index of every project.
pub async fn index(State(state): State<Arc<ServerState>>, request: Request) -> Response {
    answer(&state, String::new(), request).await
}

/// A file, a folder's index, or `files.json`, the list the index reads.
pub async fn get(
    State(state): State<Arc<ServerState>>,
    Path(path): Path<String>,
    request: Request,
) -> Response {
    answer(&state, path, request).await
}

async fn answer(state: &ServerState, path: String, request: Request) -> Response {
    let mut response = match state.sessions.artifacts_find(path).await {
        FoundArtifact::FileList(files) => Json(files).into_response(),
        FoundArtifact::File(file) => match ServeFile::new(file).oneshot(request).await {
            Ok(response) => response.into_response(),
            Err(never) => match never {},
        },
        FoundArtifact::Missing => (StatusCode::NOT_FOUND, "No such artifact").into_response(),
    };
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(SANDBOX_POLICY),
    );
    // The sandboxed pages' origin is opaque, so reading the list or a
    // module script beside them is a cross-origin request.
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );
    response
}
