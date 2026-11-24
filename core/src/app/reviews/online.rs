//! Reviews from a device: each asked of the server's API as the signed-in
//! person, its JSON answer passed back as it came, so the app needs none
//! of the analysis. Online only, as `accounts::online` is.

use reqwest::Method;
use serde_json::Value;

use crate::accounts::online::OnlineError;
use crate::app::users;
use crate::deployment::Deployment;
use crate::services::web_server::Http;

async fn ask(
    deployment: &Deployment,
    server: &Http,
    method: Method,
    path: &str,
    body: Option<&Value>,
) -> Result<Value, OnlineError> {
    let (_user, token) = users::signed_in_with_token(deployment).await?;
    Ok(server.json(method, &token, path, body).await?)
}

/// The branches and latest commits of the repository at `repo_path`.
pub async fn branches_list(
    deployment: &Deployment,
    server: &Http,
    repo_path: &str,
) -> Result<Value, OnlineError> {
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("repo_path", repo_path)
        .finish();
    let path = format!("/api/reviews/branches?{query}");
    ask(deployment, server, Method::GET, &path, None).await
}

/// What a comparison changed, by file and group.
pub async fn file_changes_list(
    deployment: &Deployment,
    server: &Http,
    request: &Value,
) -> Result<Value, OnlineError> {
    let path = "/api/reviews/file-changes";
    ask(deployment, server, Method::POST, path, Some(request)).await
}

/// The whole of each changed function, its lines marked.
pub async fn function_diffs_list(
    deployment: &Deployment,
    server: &Http,
    request: &Value,
) -> Result<Value, OnlineError> {
    let path = "/api/reviews/function-diffs";
    ask(deployment, server, Method::POST, path, Some(request)).await
}

/// The changed lines of files, with a few lines of context.
pub async fn file_diffs_list(
    deployment: &Deployment,
    server: &Http,
    request: &Value,
) -> Result<Value, OnlineError> {
    let path = "/api/reviews/file-diffs";
    ask(deployment, server, Method::POST, path, Some(request)).await
}
