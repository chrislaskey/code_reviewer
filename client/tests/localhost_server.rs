//! The client against a real server on a free localhost port.

use std::path::Path;
use std::process::Command;

use code_reviewer_client::{BackendClient, ClientError};
use code_reviewer_core::git::ListBranchesAndRecentCommitsRequest;
use code_reviewer_server::{RunningServer, ServerConfig, start};
use secrecy::SecretString;

async fn start_server(data_directory: &Path) -> RunningServer {
    start(ServerConfig::new(data_directory.to_path_buf()))
        .await
        .unwrap()
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

fn list_branches_request(repo_path: &Path) -> ListBranchesAndRecentCommitsRequest {
    ListBranchesAndRecentCommitsRequest {
        repo_path: repo_path.to_string_lossy().into_owned(),
        recent_commit_count: None,
    }
}

#[tokio::test]
async fn a_client_with_the_token_gets_the_commands_response() {
    let data_directory = tempfile::tempdir().unwrap();
    let repository = repository_with_one_commit();
    let server = start_server(data_directory.path()).await;
    let client = BackendClient::new(server.localhost_url(), server.desktop_token().clone());

    let response = client
        .list_branches_and_recent_commits(&list_branches_request(repository.path()))
        .await
        .unwrap();

    assert_eq!(response.current_branch.as_deref(), Some("main"));
    assert_eq!(response.recent_commits[0].subject, "First commit");
}

#[tokio::test]
async fn a_failed_command_comes_back_as_its_message() {
    let data_directory = tempfile::tempdir().unwrap();
    let server = start_server(data_directory.path()).await;
    let client = BackendClient::new(server.localhost_url(), server.desktop_token().clone());

    let error = client
        .list_branches_and_recent_commits(&list_branches_request(Path::new("/no/such/repository")))
        .await
        .unwrap_err();

    assert!(matches!(error, ClientError::Api { status: 500, .. }));
    assert!(!error.to_string().is_empty());
}

#[tokio::test]
async fn a_client_with_another_token_is_refused() {
    let data_directory = tempfile::tempdir().unwrap();
    let repository = repository_with_one_commit();
    let server = start_server(data_directory.path()).await;
    let client = BackendClient::new(server.localhost_url(), SecretString::from("not-the-token"));

    let error = client
        .list_branches_and_recent_commits(&list_branches_request(repository.path()))
        .await
        .unwrap_err();

    assert!(matches!(error, ClientError::Api { status: 401, .. }));
}

#[tokio::test]
async fn a_restarted_server_keeps_its_token() {
    let data_directory = tempfile::tempdir().unwrap();
    let first = start_server(data_directory.path()).await;
    let first_token = secrecy::ExposeSecret::expose_secret(first.desktop_token()).to_string();
    first.shutdown();

    let second = start_server(data_directory.path()).await;

    assert_eq!(
        secrecy::ExposeSecret::expose_secret(second.desktop_token()),
        first_token
    );
}

#[tokio::test]
async fn a_stopped_server_is_unreachable() {
    let data_directory = tempfile::tempdir().unwrap();
    let repository = repository_with_one_commit();
    let server = start_server(data_directory.path()).await;
    let client = BackendClient::new(server.localhost_url(), server.desktop_token().clone());
    server.shutdown();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let error = client
        .list_branches_and_recent_commits(&list_branches_request(repository.path()))
        .await
        .unwrap_err();

    assert!(matches!(error, ClientError::Request(_)));
}
