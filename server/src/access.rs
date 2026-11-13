//! Who may use a listener: a middleware run before every route. A request
//! must name the listener's own address in `Host`, which stops DNS
//! rebinding, and carry the listener's token in `Authorization`, which web
//! pages can't know. No CORS headers are ever sent, so browsers refuse to
//! send a page's request with that header at all.

use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::http::header::{AUTHORIZATION, HOST};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use std::sync::Arc;

use secrecy::SecretString;

use crate::http_api::ApiError;
use crate::paired_devices::PairedDevices;
use crate::tokens::tokens_match;

/// The tokens a listener accepts.
#[derive(Clone)]
pub enum AccessTokens {
    /// The localhost listener's one token, the desktop app's.
    One(SecretString),
    /// The tailnet listener's: one per paired device, read at each request,
    /// so a device revoked is refused at once.
    PairedDevices(Arc<PairedDevices>),
}

impl AccessTokens {
    fn accept(&self, token: &str) -> bool {
        match self {
            Self::One(expected) => tokens_match(token, expected),
            Self::PairedDevices(devices) => devices.find_device_for_token(token).is_some(),
        }
    }
}

/// What a listener accepts: the `Host` values that name it, and its tokens.
#[derive(Clone)]
pub struct ListenerAccess {
    pub allowed_hosts: Vec<String>,
    pub tokens: AccessTokens,
}

/// Rejects a request with an unexpected `Host` (403) or without the token (401).
pub async fn require_listener_access(
    State(access): State<ListenerAccess>,
    request: Request,
    next: Next,
) -> Response {
    let host = request
        .headers()
        .get(HOST)
        .and_then(|value| value.to_str().ok());
    if !host.is_some_and(|host| access.allowed_hosts.iter().any(|allowed| allowed == host)) {
        return ApiError::new(StatusCode::FORBIDDEN, "unexpected Host header").into_response();
    }

    let token = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));
    if !token.is_some_and(|token| access.tokens.accept(token)) {
        return ApiError::new(StatusCode::UNAUTHORIZED, "missing or wrong token").into_response();
    }

    next.run(request).await
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::body::Body;
    use axum::middleware;
    use tower::ServiceExt;

    use super::*;
    use crate::backend::Backend;
    use crate::http_api::localhost_router;
    use crate::remote_access::RemoteAccess;

    const HOST_NAME: &str = "127.0.0.1:4100";
    const TOKEN: &str = "the-token";

    async fn send(host: Option<&str>, authorization: Option<&str>) -> StatusCode {
        let access = ListenerAccess {
            allowed_hosts: vec![HOST_NAME.to_string()],
            tokens: AccessTokens::One(SecretString::from(TOKEN)),
        };
        let data_directory = tempfile::tempdir().unwrap();
        let backend = Arc::new(Backend::new());
        let remote_access =
            Arc::new(RemoteAccess::open(data_directory.path(), Arc::clone(&backend)).unwrap());
        let app = localhost_router(backend, remote_access).layer(middleware::from_fn_with_state(
            access,
            require_listener_access,
        ));
        let mut request = Request::builder()
            .method("POST")
            .uri("/api/no_such_command");
        if let Some(host) = host {
            request = request.header(HOST, host);
        }
        if let Some(authorization) = authorization {
            request = request.header(AUTHORIZATION, authorization);
        }
        let response = app
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap();
        response.status()
    }

    #[tokio::test]
    async fn a_request_with_the_host_and_token_reaches_the_routes() {
        let status = send(Some(HOST_NAME), Some("Bearer the-token")).await;

        // The route doesn't exist, so reaching the router is a 404.
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn another_host_is_forbidden_even_with_the_token() {
        let status = send(Some("evil.example.com:4100"), Some("Bearer the-token")).await;

        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn a_missing_host_is_forbidden() {
        let status = send(None, Some("Bearer the-token")).await;

        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn a_missing_token_is_unauthorized() {
        let status = send(Some(HOST_NAME), None).await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn a_wrong_token_is_unauthorized() {
        let status = send(Some(HOST_NAME), Some("Bearer not-the-token")).await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn a_token_without_the_bearer_scheme_is_unauthorized() {
        let status = send(Some(HOST_NAME), Some("the-token")).await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
}
