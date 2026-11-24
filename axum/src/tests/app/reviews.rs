//! The reviews and traces routes, on a git repository made for the test:
//! answered for an app's bearer token, never for the browser's cookie.

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
    let (router, _) = test_router().await;
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
