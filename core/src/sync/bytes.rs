//! Bytes: what a synced row may carry beside its fields, a file's or a
//! picture's, kept apart from the row, see
//! `docs/design/offline-files-decisions.md` and
//! `docs/design/template-and-apps.md`. A table whose rows carry bytes
//! implements `SyncedBytes` and answers it from `SyncedTable::bytes`;
//! everything else is here, the same for every such table.
//!
//! On the server the bytes are in the bucket at
//! `<prefix>/<creator id>/<row id>`, the prefix the table's; on a device,
//! in the account's folder under the row's id, see
//! `deployment::FileStore`. Bytes never change, and the row holds their
//! SHA-256, which the bucket checks on the upload and the server on the
//! create. A device keeps the bytes first, then writes the row and
//! queues its create; the sync asks the server about the bytes
//! (`ask_about_waiting`), holds back the creates whose bytes the server
//! lacks (`held_back`), sends those bytes last (`upload_waiting`), and
//! pushes their creates again. Another device fetches the bytes the
//! first time a page shows the row (`local_copy`). Rows go first and
//! bytes after, on both sides: `delete_unreferenced` in the bucket,
//! daily, and `delete_unreferenced_on_device` at the end of each sync.
//!
//! A row may keep small content itself instead, see
//! `docs/design/files-content.md`. Nothing of it goes to the bucket: its
//! create does not wait, a device writes the content to the folder when
//! a page asks for it, and the server answers it itself (`row_content`).

#[cfg(feature = "object-storage")]
pub mod uploads;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use garde::Validate;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::app::users::{self, UserError};
use crate::data::app_events::{self, Event, EventKind, StoredEvent};
use crate::data::app_users::User;
use crate::data::device_settings::{self, WORK_OFFLINE};
use crate::database::Transaction;
use crate::deployment::{Deployment, FileStore};
use crate::helpers::content_type::is_content_type;
use crate::helpers::field_errors::FieldErrors;
use crate::helpers::sha256::is_sha256;
#[cfg(feature = "object-storage")]
use crate::services::storage::{Storage, StorageError};
use crate::services::web_server::Transport;
use crate::sync::SyncError;
use crate::sync::synced_table::{SyncedTable, TableError};
use crate::sync::tables;

/// Why a device refuses a waiting row itself: there is nothing to send.
pub const BYTES_GONE: &str = "the file's bytes are no longer on this device";
/// Why the server leaves a create pending: the bytes are not in its
/// bucket yet, so the device sends them and pushes again.
pub const NOT_UPLOADED: &str = "file_not_uploaded";
/// Bytes no row points at are removed once they are older than this,
/// in the bucket and on a device: an upload whose row never came, bytes
/// written a moment before their row.
pub const UNREFERENCED_DAYS: i64 = 1;
/// The waiting rows one sync asks about and sends, oldest first; the
/// rest wait for the next, so a URL is used well within its lifetime
/// and the sync lock is not held for thousands of files.
pub const PER_SYNC: usize = 100;
/// How long a signed download URL works.
#[cfg(feature = "object-storage")]
pub const DOWNLOAD_URL_LIFETIME: std::time::Duration = std::time::Duration::from_secs(60 * 60);

/// The bytes a create says its row carries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BytesOf {
    pub content_type: String,
    pub size: u64,
    pub sha256: String,
}

/// A row the user may read the bytes of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Readable {
    pub created_by_id: Uuid,
    pub size: u64,
}

/// The content a row keeps itself, with its content type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowContent {
    pub content_type: String,
    pub content: String,
}

/// A row as the cleanup sees it: whose bytes they are, and whether the
/// row is deleted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BytesRow {
    pub id: Uuid,
    pub created_by_id: Uuid,
    pub deleted: bool,
}

/// A synced table whose rows carry bytes.
#[async_trait]
pub trait SyncedBytes: Send + Sync {
    /// Where the table's bytes are in the bucket, before
    /// `/<creator id>/<row id>`.
    fn key_prefix(&self) -> &'static str;

    /// The most bytes a row may carry.
    fn max_bytes(&self) -> u64;

    /// The bytes a create carries, from its `content_type`, `size` and
    /// `sha256` fields; none when it says nothing of them.
    fn bytes_of_create(&self, event: &Event) -> Option<BytesOf> {
        Some(BytesOf {
            content_type: event.changes.get("content_type")?.as_str()?.to_string(),
            size: event.changes.get("size")?.as_u64()?,
            sha256: event.changes.get("sha256")?.as_str()?.to_string(),
        })
    }

    /// The content a create says its row keeps itself, from its
    /// `content` field; none when its bytes go to the bucket.
    fn content_of_create<'a>(&self, event: &'a Event) -> Option<&'a str> {
        event.changes.get("content")?.as_str()
    }

    /// The content of a live row the user sees, when the row keeps it
    /// itself; none when its bytes are in the bucket.
    async fn content(
        &self,
        _tx: &mut Transaction,
        _user: &User,
        _id: Uuid,
    ) -> Result<Option<RowContent>, TableError> {
        Ok(None)
    }

    /// The live row, if the user sees it, with what its bytes are.
    async fn readable(
        &self,
        tx: &mut Transaction,
        user: &User,
        id: Uuid,
    ) -> Result<Option<Readable>, TableError>;

    /// Every row, deleted or not: the bytes the cleanup keeps.
    async fn list_rows(&self, tx: &mut Transaction) -> Result<Vec<BytesRow>, TableError>;

    /// Removes a row that never reached the server, on a device, with
    /// its events and those of the rows that came with its create; its
    /// bytes go once this is committed.
    async fn forget(&self, tx: &mut Transaction, id: Uuid) -> Result<(), TableError>;

    /// The rows of other tables whose pending events wait with these
    /// held-back rows: a row in a community of a file not yet on the
    /// server, which the server would answer by a row it never had.
    async fn rows_waiting_with(
        &self,
        _tx: &mut Transaction,
        _held_back: &BTreeSet<Uuid>,
    ) -> Result<Vec<Uuid>, TableError> {
        Ok(Vec::new())
    }

    /// Removes from a device the rows the user no longer sees, after a
    /// pull, but those whose create has not reached the server; the ids
    /// that went, whose bytes go once this is committed.
    async fn prune(&self, _tx: &mut Transaction, _user: &User) -> Result<Vec<Uuid>, TableError> {
        Ok(Vec::new())
    }
}

/// Bytes about to be sent to the bucket. A device names the row's id;
/// the web names none and the server makes one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Validate)]
pub struct UploadNew {
    #[garde(skip)]
    #[serde(default)]
    pub id: Option<Uuid>,
    #[garde(custom(is_content_type))]
    pub content_type: String,
    #[garde(skip)]
    pub size: u64,
    #[garde(custom(is_sha256))]
    pub sha256: String,
}

/// The server's answer to `UploadNew`: the bytes are already in the
/// bucket, or where to PUT them, with the `Content-Type` and the
/// `x-amz-checksum-sha256` headers the URL was signed for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Upload {
    pub id: Uuid,
    pub uploaded: bool,
    pub upload_url: Option<String>,
    pub content_type: String,
    /// The bytes' SHA-256 in base64, as the header carries it.
    pub checksum_sha256: String,
}

/// A row waiting on a device for its bytes to reach the server, with
/// what the server said when asked about them: nothing when it could
/// not be asked or would not say, that it has them, or where to send
/// them.
#[derive(Clone, Debug)]
pub struct Waiting {
    pub table: String,
    pub id: Uuid,
    /// The row's create, pending in the queue.
    pub create_id: Uuid,
    pub upload: Option<Upload>,
}

/// What a sweep of unreferenced bytes kept and deleted.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeleteUnreferencedReport {
    pub kept: usize,
    pub deleted: usize,
}

#[derive(Debug, Error)]
pub enum BytesError {
    #[error("{0} keeps no bytes")]
    UnknownTable(String),
    #[error("{id} not found")]
    NotFound { id: Uuid },
    #[error("{0}")]
    Invalid(FieldErrors),
    #[error("the change was refused: {0}")]
    Rejected(String),
    #[error("files are not set up on this deployment")]
    NoFileStore,
    #[error("{0}")]
    Malformed(String),
    #[error("the file is not on this device")]
    NotOnDevice,
    #[error("could not read or write the file on this device: {0}")]
    Io(#[from] std::io::Error),
    #[cfg(feature = "object-storage")]
    #[error(transparent)]
    Storage(#[from] StorageError),
    /// A synced table's own module failed.
    #[error(transparent)]
    Table(#[from] TableError),
    #[error(transparent)]
    User(#[from] UserError),
    /// The remote backend failed, or answered what this error has no
    /// variant for.
    #[cfg(feature = "remote-backend")]
    #[error(transparent)]
    Backend(#[from] crate::services::backend::BackendError),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl From<FieldErrors> for BytesError {
    fn from(errors: FieldErrors) -> BytesError {
        BytesError::Invalid(errors)
    }
}

/// The table, if its rows carry bytes.
pub fn of_table(table: &str) -> Result<&'static dyn SyncedBytes, BytesError> {
    tables::get(table)
        .and_then(|synced| synced.bytes())
        .ok_or_else(|| BytesError::UnknownTable(table.to_string()))
}

/// Every synced table whose rows carry bytes, with those bytes.
fn tables() -> impl Iterator<Item = (&'static dyn SyncedTable, &'static dyn SyncedBytes)> {
    tables::list()
        .iter()
        .filter_map(|synced| Some((*synced, synced.bytes()?)))
}

/// Where a row's bytes are in the bucket, under its creator.
pub fn storage_key(bytes: &dyn SyncedBytes, created_by_id: Uuid, id: Uuid) -> String {
    format!("{}/{created_by_id}/{id}", bytes.key_prefix())
}

/// The bucket of a server set up for files.
#[cfg(feature = "object-storage")]
pub fn bucket(deployment: &Deployment) -> Result<&Storage, BytesError> {
    match &deployment.file_store {
        FileStore::Bucket(storage) => Ok(storage),
        _ => Err(BytesError::NoFileStore),
    }
}

/// A URL the bytes of a row the user sees are read from for the next
/// hour, signed by the server, with the `Content-Disposition` the
/// bucket answers it with, if any. A row that keeps its content itself
/// has none: see `row_content`.
#[cfg(feature = "object-storage")]
pub async fn download_url(
    deployment: &Deployment,
    user: &User,
    table: &str,
    id: Uuid,
    content_disposition: Option<&str>,
) -> Result<String, BytesError> {
    let bytes = of_table(table)?;
    let storage = bucket(deployment)?;
    #[cfg(feature = "remote-backend")]
    if let Some(backend) = &deployment.remote_backend {
        let readable = readable_from_backend(backend, user, table, id).await?;
        return Ok(storage
            .download_url(
                &readable.storage_key,
                DOWNLOAD_URL_LIFETIME,
                content_disposition,
            )
            .await?);
    }
    let mut tx = deployment.begin().await?;
    let readable = bytes
        .readable(&mut tx, user, id)
        .await?
        .ok_or(BytesError::NotFound { id })?;
    tx.commit().await?;
    Ok(storage
        .download_url(
            &storage_key(bytes, readable.created_by_id, id),
            DOWNLOAD_URL_LIFETIME,
            content_disposition,
        )
        .await?)
}

/// The content of a row the user sees, when the row keeps it itself:
/// what the server answers instead of a redirect to the bucket. None
/// when its bytes are in the bucket.
pub async fn row_content(
    deployment: &Deployment,
    user: &User,
    table: &str,
    id: Uuid,
) -> Result<Option<RowContent>, BytesError> {
    let bytes = of_table(table)?;
    #[cfg(feature = "remote-backend")]
    if let Some(backend) = &deployment.remote_backend {
        let readable = readable_from_backend(backend, user, table, id).await?;
        return Ok(readable.content.map(|content| RowContent {
            content_type: readable.content_type,
            content,
        }));
    }
    let mut tx = deployment.begin().await?;
    bytes
        .readable(&mut tx, user, id)
        .await?
        .ok_or(BytesError::NotFound { id })?;
    let content = bytes.content(&mut tx, user, id).await?;
    tx.commit().await?;
    Ok(content)
}

/// What Rust needs of a row the user sees to read its bytes, from the
/// remote backend: its name, its content type, its key in the bucket,
/// and the content the row keeps itself, if any, see
/// `docs/design/backend-protocol.md`.
#[cfg(feature = "remote-backend")]
#[derive(Debug, Deserialize)]
pub struct ReadableFromBackend {
    pub name: String,
    pub content_type: String,
    pub storage_key: String,
    pub content: Option<String>,
}

/// `<resource>/readable` of the table's resource, its name without
/// `app_`: `files/readable` for `app_files`.
#[cfg(feature = "remote-backend")]
pub async fn readable_from_backend(
    backend: &crate::services::backend::RemoteBackend,
    user: &User,
    table: &str,
    id: Uuid,
) -> Result<ReadableFromBackend, BytesError> {
    let resource = table.strip_prefix("app_").unwrap_or(table);
    let request = serde_json::json!({ "actor_id": user.id, "id": id });
    match backend
        .call(&format!("{resource}/readable"), &request)
        .await
    {
        Ok(readable) => Ok(readable),
        Err(error) if error.code() == Some("not_found") => Err(BytesError::NotFound { id }),
        Err(error) => Err(error.into()),
    }
}

/// The bytes of a row the user sees, read from the bucket: what the
/// server in this process answers a device's fetch with in tests.
#[cfg(feature = "object-storage")]
pub(crate) async fn get_from_bucket(
    deployment: &Deployment,
    user: &User,
    table: &str,
    id: Uuid,
) -> Result<Vec<u8>, BytesError> {
    let bytes = of_table(table)?;
    #[cfg(feature = "remote-backend")]
    if let Some(backend) = &deployment.remote_backend {
        let readable = readable_from_backend(backend, user, table, id).await?;
        return Ok(bucket(deployment)?.get(&readable.storage_key).await?);
    }
    let mut tx = deployment.begin().await?;
    let readable = bytes
        .readable(&mut tx, user, id)
        .await?
        .ok_or(BytesError::NotFound { id })?;
    tx.commit().await?;
    Ok(bucket(deployment)?
        .get(&storage_key(bytes, readable.created_by_id, id))
        .await?)
}

/// Whether the bucket holds the bytes the create says its row carries,
/// at that size and with that checksum: what the server asks before it
/// applies the create. A row that keeps its content itself has nothing
/// to wait for.
pub async fn is_uploaded(
    deployment: &Deployment,
    user: &User,
    bytes: &dyn SyncedBytes,
    event: &Event,
) -> Result<bool, BytesError> {
    if bytes.content_of_create(event).is_some() {
        return Ok(true);
    }
    let Some(of) = bytes.bytes_of_create(event) else {
        return Ok(false);
    };
    let key = storage_key(bytes, user.id, event.row_id);
    bucket_has_bytes(deployment, &key, of.size, &of.sha256).await
}

#[cfg(feature = "object-storage")]
async fn bucket_has_bytes(
    deployment: &Deployment,
    key: &str,
    size: u64,
    sha256: &str,
) -> Result<bool, BytesError> {
    let Ok(checksum_sha256) = base64_of_hex(sha256) else {
        return Ok(false);
    };
    bucket_has(bucket(deployment)?, key, size, &checksum_sha256).await
}

/// Without the bucket's code compiled in, as in the apps, there is none.
#[cfg(not(feature = "object-storage"))]
async fn bucket_has_bytes(
    _deployment: &Deployment,
    _key: &str,
    _size: u64,
    _sha256: &str,
) -> Result<bool, BytesError> {
    Err(BytesError::NoFileStore)
}

/// Whether the bucket holds bytes at the key of the size, whose checksum
/// it checked is this base64 SHA-256.
#[cfg(feature = "object-storage")]
async fn bucket_has(
    storage: &Storage,
    key: &str,
    size: u64,
    checksum_sha256: &str,
) -> Result<bool, BytesError> {
    if storage.size(key).await? != Some(size) {
        return Ok(false);
    }
    Ok(storage.checksum_sha256(key).await?.as_deref() == Some(checksum_sha256))
}

/// The checksum as the bucket's header carries it, base64, from the
/// row's hex.
#[cfg(feature = "object-storage")]
fn base64_of_hex(sha256: &str) -> Result<String, BytesError> {
    use base64::Engine;

    let bytes = (0..sha256.len())
        .step_by(2)
        .map(|at| {
            sha256
                .get(at..at + 2)
                .and_then(|pair| u8::from_str_radix(pair, 16).ok())
        })
        .collect::<Option<Vec<u8>>>()
        .filter(|_| is_sha256(sha256, &()).is_ok())
        .ok_or_else(|| BytesError::Malformed(format!("{sha256} is not a SHA-256")))?;
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}

/// The first step of a sync on a device: asks the server about the
/// bytes of the first `PER_SYNC` rows whose create is pending, but those
/// that keep their content themselves, and
/// answers every one, those not asked with nothing. What the device
/// knows of an upload is only this answer, asked again by every sync
/// and kept nowhere. Once the server cannot be reached the rest are not
/// asked, so a device offline waits for one request, not one per row.
pub async fn ask_about_waiting<T: Transport>(
    deployment: &Deployment,
    transport: &T,
    token: Option<&str>,
) -> Result<Vec<Waiting>, SyncError> {
    let mut tx = deployment.begin().await?;
    let creates: Vec<(&'static dyn SyncedBytes, StoredEvent)> = app_events::pending(&mut tx)
        .await?
        .into_iter()
        .filter(|stored| stored.event.kind == EventKind::Create)
        .filter_map(|stored| Some((of_table(&stored.event.table).ok()?, stored)))
        .filter(|(bytes, stored)| bytes.content_of_create(&stored.event).is_none())
        .collect();
    tx.commit().await?;

    let mut waiting = Vec::with_capacity(creates.len());
    let mut reachable = true;
    for (index, (bytes, stored)) in creates.into_iter().enumerate() {
        let event = &stored.event;
        let mut row = Waiting {
            table: event.table.clone(),
            id: event.row_id,
            create_id: event.id,
            upload: None,
        };
        let of = bytes.bytes_of_create(event);
        let request = UploadNew {
            id: Some(event.row_id),
            content_type: of
                .as_ref()
                .map(|of| of.content_type.clone())
                .unwrap_or_default(),
            size: of.as_ref().map(|of| of.size).unwrap_or_default(),
            sha256: of.map(|of| of.sha256).unwrap_or_default(),
        };
        if reachable && index < PER_SYNC {
            match transport
                .bytes_uploads_create(token, &event.table, request)
                .await
            {
                Ok(upload) => row.upload = Some(upload),
                // The server will not take these bytes, whatever is sent
                Err(SyncError::Refused(reason)) => refuse(deployment, event.id, &reason).await?,
                Err(error @ SyncError::Transport(_)) => {
                    reachable = false;
                    note_error(deployment, event.id, &error.to_string()).await?;
                }
                Err(error) => return Err(error),
            }
        }
        waiting.push(row);
    }
    Ok(waiting)
}

/// The rows whose events are held back from the push while their bytes
/// wait: those whose bytes the server does not have, and the rows of
/// other tables that wait with them. A refused row's are held back too,
/// see `conflicts::hold_back_refused`.
pub async fn held_back(
    deployment: &Deployment,
    waiting: &[Waiting],
) -> Result<BTreeSet<Uuid>, SyncError> {
    let mut held_back: BTreeSet<Uuid> = waiting
        .iter()
        .filter(|row| !row.upload.as_ref().is_some_and(|upload| upload.uploaded))
        .map(|row| row.id)
        .collect();
    let mut tx = deployment.begin().await?;
    let mut with_them = Vec::new();
    for (_, bytes) in tables() {
        with_them.extend(bytes.rows_waiting_with(&mut tx, &held_back).await?);
    }
    held_back.extend(with_them);
    tx.commit().await?;
    Ok(held_back)
}

/// The last step of a sync on a device before the waiting rows' events
/// are pushed: sends the bytes of each waiting row the server gave a URL
/// for, from the account's folder, and marks it uploaded. How many went.
/// A row whose bytes are gone from the device is refused; one the bucket
/// says no to counts a try and waits for the next sync; once the bucket
/// cannot be reached the rest wait too, and nothing counts.
///
/// This runs inside the sync, which holds the sync lock while bytes go.
/// An application with large files calls it from a task of its own
/// instead, followed by a sync.
pub async fn upload_waiting<T: Transport>(
    deployment: &Deployment,
    transport: &T,
    token: Option<&str>,
    waiting: &mut [Waiting],
) -> Result<usize, SyncError> {
    let mut uploaded = 0;
    for row in waiting {
        let Some(upload) = row.upload.as_mut().filter(|upload| !upload.uploaded) else {
            continue;
        };
        let path = local_path(deployment, row.id).map_err(SyncError::from)?;
        if !tokio::fs::try_exists(&path).await.unwrap_or(false) {
            refuse(deployment, row.create_id, BYTES_GONE).await?;
            continue;
        }
        match transport
            .bytes_upload(token, &row.table, upload, &path)
            .await
        {
            Ok(()) => {
                upload.uploaded = true;
                uploaded += 1;
            }
            Err(SyncError::Refused(reason)) => {
                let mut tx = deployment.begin().await?;
                app_events::count_attempt(&mut tx, row.create_id, &reason).await?;
                tx.commit().await?;
            }
            Err(error @ SyncError::Transport(_)) => {
                note_error(deployment, row.create_id, &error.to_string()).await?;
                break;
            }
            Err(error) => return Err(error),
        }
    }
    Ok(uploaded)
}

/// Stops trying a waiting row, with why: kept for the person.
async fn refuse(deployment: &Deployment, create_id: Uuid, reason: &str) -> Result<(), SyncError> {
    let mut tx = deployment.begin().await?;
    app_events::refuse(&mut tx, create_id, reason).await?;
    tx.commit().await?;
    Ok(())
}

/// Keeps why a waiting row's bytes did not go, without counting a try.
async fn note_error(
    deployment: &Deployment,
    create_id: Uuid,
    reason: &str,
) -> Result<(), SyncError> {
    let mut tx = deployment.begin().await?;
    app_events::mark_error(&mut tx, create_id, reason).await?;
    tx.commit().await?;
    Ok(())
}

/// Removes from a device the rows its user no longer sees, table by
/// table, and their bytes; how many rows went, and how many of their
/// bytes were here to remove.
pub(crate) async fn prune(deployment: &Deployment, user: &User) -> Result<(u64, usize), SyncError> {
    let mut tx = deployment.begin().await?;
    let mut pruned = Vec::new();
    for (_, bytes) in tables() {
        pruned.extend(bytes.prune(&mut tx, user).await?);
    }
    tx.commit().await?;
    let mut cleaned = 0;
    for id in &pruned {
        // A deployment with no folder has no bytes to remove
        let Ok(path) = local_path(deployment, *id) else {
            break;
        };
        if tokio::fs::try_exists(&path).await.unwrap_or(false) {
            delete_local(&path).await.map_err(SyncError::from)?;
            cleaned += 1;
        }
    }
    Ok((pruned.len() as u64, cleaned))
}

/// Deletes from the bucket the bytes no row points at, deleted rows
/// included, once they are older than `UNREFERENCED_DAYS`: an upload
/// whose push never came, a refused row let go, a row the purge
/// removed. A deleted row keeps its bytes until the purge takes it. On
/// the server, daily after `purge::run`, see `jobs`.
///
/// A device that comes back in the moment between this reading the rows
/// and deleting the bytes can have its row written just before the bytes
/// go; rare enough to accept, see "A gap accepted" in
/// `docs/design/offline-files-decisions.md`.
#[cfg(feature = "object-storage")]
pub async fn delete_unreferenced(
    deployment: &Deployment,
    now: DateTime<Utc>,
) -> Result<DeleteUnreferencedReport, BytesError> {
    let storage = bucket(deployment)?;
    let cutoff = now - chrono::Duration::days(UNREFERENCED_DAYS);
    let mut report = DeleteUnreferencedReport::default();
    for (table, bytes) in tables() {
        let referenced = referenced_keys(deployment, table.table(), bytes).await?;
        for stored in storage.list(bytes.key_prefix()).await? {
            if referenced.contains(&stored.key) || stored.last_modified > cutoff {
                report.kept += 1;
                continue;
            }
            storage.delete(&stored.key).await?;
            report.deleted += 1;
        }
    }
    Ok(report)
}

/// The bucket keys of the table's rows, deleted or not, which the sweep
/// keeps: from the remote backend's `<resource>/storage_keys_list`, when
/// it owns the rows.
#[cfg(feature = "object-storage")]
async fn referenced_keys(
    deployment: &Deployment,
    #[cfg_attr(not(feature = "remote-backend"), expect(unused_variables))] table: &str,
    bytes: &dyn SyncedBytes,
) -> Result<BTreeSet<String>, BytesError> {
    #[cfg(feature = "remote-backend")]
    if let Some(backend) = &deployment.remote_backend {
        #[derive(Deserialize)]
        struct Keys {
            keys: BTreeSet<String>,
        }
        let resource = table.strip_prefix("app_").unwrap_or(table);
        let operation = format!("{resource}/storage_keys_list");
        let keys: Keys = backend.call(&operation, &serde_json::json!({})).await?;
        return Ok(keys.keys);
    }
    let mut tx = deployment.begin().await?;
    let referenced = bytes
        .list_rows(&mut tx)
        .await?
        .into_iter()
        .map(|row| storage_key(bytes, row.created_by_id, row.id))
        .collect();
    tx.commit().await?;
    Ok(referenced)
}

/// Removes from the account's folder the bytes no live row points at:
/// those of a deleted row at once, and those of no row, or a fetch left
/// half done, once they are older than `UNREFERENCED_DAYS`, since a row
/// being created has its bytes there a moment before it. That covers a
/// delete, a resync that dropped the rows, and a crash between the
/// bytes and the row. How many went. A deployment with no folder has
/// nothing to remove.
pub async fn delete_unreferenced_on_device(
    deployment: &Deployment,
    now: DateTime<Utc>,
) -> Result<usize, BytesError> {
    let FileStore::Folder(folder) = &deployment.file_store else {
        return Ok(0);
    };
    let mut tx = deployment.begin().await?;
    let mut rows = Vec::new();
    for (_, bytes) in tables() {
        rows.extend(bytes.list_rows(&mut tx).await?);
    }
    tx.commit().await?;

    let cutoff = now - chrono::Duration::days(UNREFERENCED_DAYS);
    let mut removed = 0;
    let mut entries = tokio::fs::read_dir(folder).await?;
    while let Some(entry) = entries.next_entry().await? {
        let row = entry
            .file_name()
            .to_str()
            .and_then(|name| Uuid::parse_str(name).ok())
            .and_then(|id| rows.iter().find(|row| row.id == id));
        let remove = match row {
            Some(row) => row.deleted,
            None => {
                let modified: DateTime<Utc> = entry.metadata().await?.modified()?.into();
                modified < cutoff
            }
        };
        if remove {
            delete_local(&entry.path()).await?;
            removed += 1;
        }
    }
    Ok(removed)
}

/// Where the bytes of a row the user sees are on this device, fetched
/// from the server first when they are not there: the first time a
/// device that did not create the row shows it. Fetched under a
/// temporary name and renamed once whole, so bytes at their path are
/// never half there. Working offline, or with the server out of reach,
/// bytes not here are `NotOnDevice`. A row that keeps its content itself
/// has it written here, with no fetch.
pub async fn local_copy<T: Transport>(
    deployment: &Deployment,
    transport: &T,
    user: &User,
    table: &str,
    id: Uuid,
) -> Result<PathBuf, BytesError> {
    let bytes = of_table(table)?;
    let mut tx = deployment.begin().await?;
    let readable = bytes
        .readable(&mut tx, user, id)
        .await?
        .ok_or(BytesError::NotFound { id })?;
    let content = bytes.content(&mut tx, user, id).await?;
    let offline: bool = device_settings::get(&mut tx, WORK_OFFLINE)
        .await
        .map_err(|error| BytesError::Malformed(error.to_string()))?;
    tx.commit().await?;
    let path = local_path(deployment, id)?;
    if tokio::fs::try_exists(&path).await? {
        return Ok(path);
    }
    // A name of its own, since two pages may ask for the same bytes at once
    let partial = path.with_extension(format!("{}.part", Uuid::now_v7().simple()));
    if let Some(content) = content {
        tokio::fs::write(&partial, content.content.as_bytes()).await?;
        tokio::fs::rename(&partial, &path).await?;
        return Ok(path);
    }
    if offline {
        return Err(BytesError::NotOnDevice);
    }
    let (_user, token) = users::signed_in_with_token(deployment).await?;
    let fetched = match transport
        .bytes_download(Some(&token), table, id, &partial)
        .await
    {
        Ok(()) => tokio::fs::metadata(&partial)
            .await
            .map(|fetched| fetched.len()),
        Err(SyncError::Transport(_)) => {
            delete_local(&partial).await?;
            return Err(BytesError::NotOnDevice);
        }
        Err(error) => {
            delete_local(&partial).await?;
            return Err(BytesError::Malformed(error.to_string()));
        }
    };
    if fetched.as_ref().ok() != Some(&readable.size) {
        delete_local(&partial).await?;
        return Err(BytesError::NotOnDevice);
    }
    tokio::fs::rename(&partial, &path).await?;
    Ok(path)
}

/// Removes a row that never reached the server from the device, with
/// its events, then its bytes: a refused create let go.
pub async fn forget(deployment: &Deployment, table: &str, id: Uuid) -> Result<(), BytesError> {
    let bytes = of_table(table)?;
    let mut tx = deployment.begin().await?;
    bytes.forget(&mut tx, id).await?;
    tx.commit().await?;
    delete_local_copy(deployment, id).await
}

/// Removes a row's bytes from this device, if they are there.
pub async fn delete_local_copy(deployment: &Deployment, id: Uuid) -> Result<(), BytesError> {
    delete_local(&local_path(deployment, id)?).await
}

/// Where a row's bytes are on this device, there or not.
pub fn local_path(deployment: &Deployment, id: Uuid) -> Result<PathBuf, BytesError> {
    match &deployment.file_store {
        FileStore::Folder(folder) => Ok(folder.join(id.hyphenated().to_string())),
        _ => Err(BytesError::NoFileStore),
    }
}

/// Writes the bytes under a temporary name beside the path, then
/// renames, so bytes at their path are always whole.
pub async fn write_whole(path: &Path, bytes: &[u8]) -> Result<(), BytesError> {
    let partial = partial_path(path);
    tokio::fs::write(&partial, bytes).await?;
    tokio::fs::rename(&partial, path).await?;
    Ok(())
}

/// The name bytes are written under until they are whole.
pub(crate) fn partial_path(path: &Path) -> PathBuf {
    path.with_extension("part")
}

/// Removes the bytes at the path, if any are there.
pub async fn delete_local(path: &Path) -> Result<(), BytesError> {
    match tokio::fs::remove_file(path).await {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error.into()),
        _ => Ok(()),
    }
}
