//! Traces from a device: each asked of the server's API as the signed-in
//! person, its JSON answer passed back as it came. Online only, as
//! `accounts::online` is.

use reqwest::Method;
use serde_json::Value;

use crate::accounts::online::OnlineError;
use crate::app::users;
use crate::deployment::Deployment;
use crate::services::web_server::Http;

async fn ask(
    deployment: &Deployment,
    server: &Http,
    path: &str,
    request: &Value,
) -> Result<Value, OnlineError> {
    let (_user, token) = users::signed_in_with_token(deployment).await?;
    Ok(server
        .json(Method::POST, &token, path, Some(request))
        .await?)
}

/// The tests of a comparison's head, marking the ones it added or modified.
pub async fn tests_list(
    deployment: &Deployment,
    server: &Http,
    request: &Value,
) -> Result<Value, OnlineError> {
    ask(deployment, server, "/api/traces/tests", request).await
}

/// The calls below one test.
pub async fn call_stacks_get(
    deployment: &Deployment,
    server: &Http,
    request: &Value,
) -> Result<Value, OnlineError> {
    ask(deployment, server, "/api/traces/call-stacks", request).await
}
