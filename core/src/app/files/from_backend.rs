//! Files answered by the remote backend, see the files operations of
//! `docs/design/backend-protocol.md`. Rust keeps the bucket: before a
//! create it checks the bytes arrived, and names the create `uploaded`.

use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use uuid::Uuid;

use super::{FileError, FileFilter, FileList, FileNew, FileWithState, TABLE};
use crate::data::app_events::Event;
use crate::data::app_files::File;
use crate::data::app_users::User;
use crate::deployment::Deployment;
use crate::services::backend::{BackendError, RemoteBackend};
use crate::sync::bytes;

pub(super) async fn list(
    backend: &RemoteBackend,
    user: &User,
    filter: &FileFilter,
) -> Result<FileList, FileError> {
    call(
        backend,
        "files/list",
        json!({ "actor_id": user.id, "filter": filter }),
        None,
    )
    .await
}

pub(super) async fn get(
    backend: &RemoteBackend,
    user: &User,
    id: Uuid,
) -> Result<FileWithState, FileError> {
    call(
        backend,
        "files/get",
        json!({ "actor_id": user.id, "id": id }),
        Some(id),
    )
    .await
}

/// `event` is the create the Rust server would apply, whose bytes are
/// checked as it checks them.
pub(super) async fn create(
    deployment: &Deployment,
    backend: &RemoteBackend,
    user: &User,
    new: &FileNew,
    event: &Event,
) -> Result<File, FileError> {
    let uploaded = bytes::is_uploaded(deployment, user, bytes::of_table(TABLE)?, event).await?;
    let request = json!({ "actor_id": user.id, "file": new, "uploaded": uploaded });
    call(backend, "files/create", request, Some(new.id)).await
}

pub(super) async fn update(
    backend: &RemoteBackend,
    user: &User,
    id: Uuid,
    name: &str,
    community_ids: &[Uuid],
) -> Result<File, FileError> {
    let request = json!({
        "actor_id": user.id,
        "id": id,
        "name": name,
        "community_ids": community_ids,
    });
    call(backend, "files/update", request, Some(id)).await
}

pub(super) async fn delete(
    backend: &RemoteBackend,
    user: &User,
    id: Uuid,
) -> Result<(), FileError> {
    let _: Value = call(
        backend,
        "files/delete",
        json!({ "actor_id": user.id, "id": id }),
        Some(id),
    )
    .await?;
    Ok(())
}

pub(super) async fn duplicates_list(
    backend: &RemoteBackend,
    user: &User,
    sha256s: &[String],
) -> Result<Vec<FileWithState>, FileError> {
    let request = json!({ "actor_id": user.id, "sha256s": sha256s });
    call(backend, "files/duplicates_list", request, None).await
}

async fn call<Answer: DeserializeOwned>(
    backend: &RemoteBackend,
    operation: &str,
    request: Value,
    id: Option<Uuid>,
) -> Result<Answer, FileError> {
    backend
        .call(operation, &request)
        .await
        .map_err(|error| file_error(error, id))
}

/// A file's error for each code the protocol gives it, as the Rust
/// server answers the same refusal.
fn file_error(error: BackendError, id: Option<Uuid>) -> FileError {
    match (&error, id) {
        (BackendError::Answered { code, .. }, Some(id)) if code == "not_found" => {
            FileError::NotFound { id }
        }
        (
            BackendError::Answered {
                fields: Some(fields),
                ..
            },
            _,
        ) => FileError::Invalid(fields.clone()),
        (BackendError::Answered { code, .. }, _) if code == "not_a_member" || code == "exists" => {
            FileError::Rejected(code.clone())
        }
        (BackendError::Answered { code, message, .. }, _) if code == "invalid" => {
            FileError::Rejected(message.clone())
        }
        _ => FileError::Backend(error),
    }
}
