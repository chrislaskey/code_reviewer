//! Push, pull and backfill answered by the remote backend, see
//! `docs/design/backend-protocol.md`. Rust keeps the bucket, so before a
//! push it checks each create of a row with bytes, as the Rust server
//! does, and names those whose bytes arrived as `uploaded`.

use serde::Serialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::data::app_events::EventKind;
use crate::data::app_users::User;
use crate::deployment::Deployment;
use crate::services::backend::{BackendError, RemoteBackend};
use crate::sync::{
    BackfillRequest, Page, PullRequest, PushRequest, PushResponse, SyncError, bytes,
};

pub(super) async fn push(
    deployment: &Deployment,
    backend: &RemoteBackend,
    user: &User,
    request: PushRequest,
) -> Result<PushResponse, SyncError> {
    let mut uploaded: Vec<Uuid> = Vec::new();
    for event in &request.events {
        if event.kind != EventKind::Create {
            continue;
        }
        let Ok(with_bytes) = bytes::of_table(&event.table) else {
            continue;
        };
        if bytes::is_uploaded(deployment, user, with_bytes, event).await? {
            uploaded.push(event.id);
        }
    }
    let request = json!({
        "actor_id": user.id,
        "events": request.events,
        "uploaded": uploaded,
    });
    call(backend, "sync/push", &request, None).await
}

pub(super) async fn pull(
    backend: &RemoteBackend,
    user: &User,
    request: PullRequest,
) -> Result<Page, SyncError> {
    call(backend, "sync/pull", &with_actor(user, &request)?, None).await
}

pub(super) async fn backfill(
    backend: &RemoteBackend,
    user: &User,
    request: BackfillRequest,
) -> Result<Page, SyncError> {
    let community_id = request.community_id;
    let request = with_actor(user, &request)?;
    call(backend, "sync/backfill", &request, Some(community_id)).await
}

/// The request with the user it is for, as `actor_id`.
fn with_actor(user: &User, request: &impl Serialize) -> Result<Value, SyncError> {
    let mut value =
        serde_json::to_value(request).map_err(|error| SyncError::Malformed(error.to_string()))?;
    if let Value::Object(fields) = &mut value {
        fields.insert("actor_id".to_string(), json!(user.id));
    }
    Ok(value)
}

async fn call<Answer: serde::de::DeserializeOwned>(
    backend: &RemoteBackend,
    operation: &str,
    request: &Value,
    community_id: Option<Uuid>,
) -> Result<Answer, SyncError> {
    backend
        .call(operation, request)
        .await
        .map_err(|error| sync_error(error, community_id))
}

/// Sync's error for each code the protocol gives it.
fn sync_error(error: BackendError, community_id: Option<Uuid>) -> SyncError {
    match (error.code(), community_id) {
        (Some("invalid_continuation"), _) => SyncError::InvalidContinuation,
        (Some("not_a_member"), Some(community_id)) => SyncError::NotAMember { community_id },
        _ => SyncError::Backend(error),
    }
}
