//! The host routes, reviews, traces and sessions: answered for an app's
//! bearer token, never for the browser's cookie; reviews and traces on a
//! git repository made for the test.

use std::path::Path;
use std::process::Command;

use super::*;

/// A repository with two commits, the second changing a Rust function.
fn repository_with_two_commits(directory: &Path) {
    let git = |args: &[&str]| {
        let status = Command::new("git")
            .args(args)
            .current_dir(directory)
            .status()
            .expect("git runs");
        assert!(status.success(), "git {args:?}");
    };
    git(&["init", "--quiet", "--initial-branch", "main"]);
    git(&["config", "user.email", "test@example.com"]);
    git(&["config", "user.name", "Test"]);
    fs::write(directory.join("lib.rs"), "fn total() -> u32 {\n    1\n}\n").expect("written");
    git(&["add", "."]);
    git(&["commit", "--quiet", "-m", "First"]);
    fs::write(directory.join("lib.rs"), "fn total() -> u32 {\n    2\n}\n").expect("written");
    git(&["commit", "--quiet", "-am", "Second"]);
}

#[tokio::test]
async fn reviews_routes_answer_a_bearer_token_and_refuse_the_cookie() {
    let router = test_router().await;
    let directory = tempfile::tempdir().expect("a directory");
    repository_with_two_commits(directory.path());
    let repo_path = directory.path().to_string_lossy().to_string();
    // A temporary directory's path needs no escaping in a query
    let branches_path = format!("/api/reviews/branches?repo_path={repo_path}");

    let (status, branches) = get(&router, &branches_path).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(branches["current_branch"], json!("main"));
    assert_eq!(branches["recent_commits"][0]["subject"], json!("Second"));

    let cookie = format!("token={}", router.token);
    let (status, _, _) = send_with(
        &router.router,
        None,
        Some(&cookie),
        Method::GET,
        &branches_path,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "the cookie is not read");

    let comparison = json!({ "repo_path": repo_path, "base_ref": "HEAD~1", "head_ref": "HEAD" });
    let (status, changes) = send(
        &router,
        Method::POST,
        "/api/reviews/file-changes",
        Some(comparison.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(changes["files"][0]["file_path"], json!("lib.rs"));

    let (status, tests) = send(&router, Method::POST, "/api/traces/tests", Some(comparison)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tests["tests"], json!([]));

    let unknown = json!({ "repo_path": repo_path, "base_ref": "nope", "head_ref": "HEAD" });
    let (status, body) = send(
        &router,
        Method::POST,
        "/api/reviews/file-changes",
        Some(unknown),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body, json!({ "error": "invalid revision: nope" }));
}

#[tokio::test]
async fn sessions_routes_refuse_the_cookie_and_the_terminal_window_refuses_the_tailnet() {
    let router = test_router().await;
    let cookie = format!("token={}", router.token);
    let (status, _, _) = send_with(
        &router.router,
        None,
        Some(&cookie),
        Method::GET,
        "/api/sessions",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "the cookie is not read");

    // As `tailscale serve` passes them on: a person's device, a tagged
    // device, which has no login header, and a forwarded request naming
    // localhost.
    let tailnet_host = "laptop.example.ts.net:4000";
    for (name, headers) in [
        (
            "a person's device",
            vec![
                ("Host", tailnet_host),
                ("Tailscale-User-Login", "someone@example.com"),
                ("X-Forwarded-For", "100.64.0.1"),
            ],
        ),
        (
            "a tagged device",
            vec![("Host", tailnet_host), ("X-Forwarded-For", "100.64.0.2")],
        ),
        (
            "a forwarded request naming localhost",
            vec![
                ("Host", "localhost:4000"),
                ("X-Forwarded-For", "100.64.0.3"),
            ],
        ),
    ] {
        let mut request = Request::builder()
            .method(Method::POST)
            .uri("/api/sessions/cr-anything/terminal-window")
            .header(header::AUTHORIZATION, format!("Bearer {}", router.token));
        for (header_name, value) in headers {
            request = request.header(header_name, value);
        }
        let response = router
            .router
            .clone()
            .oneshot(request.body(Body::empty()).expect("a request"))
            .await
            .expect("a response");
        assert_eq!(response.status(), StatusCode::FORBIDDEN, "{name}");
    }
}

#[tokio::test]
async fn artifacts_answer_without_a_token_and_sandboxed() {
    let router = test_router().await;

    // The index committed in the main checkout of the repository the tests run in.
    for (path, status) in [
        ("/artifacts/", StatusCode::OK),
        ("/artifacts/.gitignore", StatusCode::NOT_FOUND),
    ] {
        let request = Request::builder()
            .uri(path)
            .body(Body::empty())
            .expect("a request");
        let response = router
            .router
            .clone()
            .oneshot(request)
            .await
            .expect("a response");
        assert_eq!(response.status(), status, "{path}");
        assert_eq!(
            response.headers()[header::CONTENT_SECURITY_POLICY],
            "sandbox allow-scripts allow-popups allow-downloads",
            "{path}"
        );
    }
}
