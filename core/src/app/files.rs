//! Files: a file in communities, as a row that syncs like a post and
//! bytes kept apart from it, see `docs/design/offline-files-decisions.md`
//! and `docs/design/files-communities.md`. The row holds the name, the
//! content type and the size; the bytes are in the server's bucket at
//! `files/<creator id>/<file id>` and, on a device, in the account's
//! folder under the file's id, see `deployment::FileStore`. The bytes
//! never change: an update changes the name only, and the row holds
//! their SHA-256, which the bucket checks on the upload and the server
//! on the create. A file belongs to its
//! communities, `files_communities`: a live member of one sees it, and
//! it lives as long as it is in one.
//!
//! Every write is an event applied by `apply_event`, as a post's is.
//! What differs is the create: it carries the file's communities, and
//! the server writes the rows only once the file is in its bucket, and
//! answers `file_not_uploaded` until then.
//! On the web the page asks `create_upload` where to PUT the bytes,
//! sends them, then creates the row. A device keeps the bytes itself,
//! `create_with_bytes`, and the file waits there, its create pending,
//! until a sync has sent them. Another device gets the row by the pull
//! and the bytes the first time a page shows the file, `local_copy`,
//! and keeps them.
//!
//! Rows go first and bytes after, on both sides, by looking at what the
//! rows point at now: `delete_unreferenced` in the bucket, daily, and
//! `delete_unreferenced_on_device` in the account's folder, at the end
//! of each sync.

use crate::data::app_files;
use crate::data::app_files_communities;
use crate::data::app_users_communities;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use garde::Validate;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;
use uuid::Uuid;

use crate::app::communities;
use crate::app::events::{
    self, Answer, AnswerStatus, Conflicts, Event, EventKind, EventStatus, Fields, StoredEvent,
};
use crate::app::files_communities::{self, FileCommunityError, FileCommunityNew};
use crate::app::sync::{Cursor, SyncError, TableRow, Transport};
use crate::app::users;
use crate::data::app_communities::Community;
use crate::data::app_users::User;
use crate::database::Transaction;
use crate::deployment::{Deployment, FileStore, Role, now};
use crate::device_settings::{self, WORK_OFFLINE};
use crate::helpers::field_errors::FieldErrors;
#[cfg(feature = "object-storage")]
use crate::helpers::{json, sha256};
use crate::services::storage::{Storage, StorageError};

/// A row accepted by this resource's event, lifecycle or replication workflow.
/// Only this module can construct an accepted write.
pub(crate) struct AcceptedFile<'a>(&'a File);

impl AcceptedFile<'_> {
    /// Borrows the accepted row for persistence.
    pub(crate) fn row(&self) -> &File {
        self.0
    }
}

pub const TABLE: &str = "app_files";
/// The largest file, 25 MB.
pub const MAX_BYTES: u64 = 25 * 1024 * 1024;
pub const MAX_NAME_LENGTH: usize = 255;
/// What a file with no content type of its own is sent as.
pub const UNKNOWN_CONTENT_TYPE: &str = "application/octet-stream";
/// Why a device refuses a waiting file itself: there is nothing to send.
pub const BYTES_GONE: &str = "the file's bytes are no longer on this device";
/// Why the server leaves a create pending: the bytes are not in its
/// bucket yet, so the device sends them and pushes again.
pub const FILE_NOT_UPLOADED: &str = "file_not_uploaded";
/// The fields an event may set when it creates a file: `communities` is
/// the rows of its first communities, written with it.
const CREATE_FIELDS: &[&str] = &["name", "content_type", "size", "sha256", "communities"];
/// The fields an event may change on a file. A file is deleted by
/// taking it out of its last community, see `files_communities`.
const UPDATE_FIELDS: &[&str] = &["name"];
/// Where files go in the bucket: `files/<creator id>/<file id>`.
const KEY_PREFIX: &str = "files";
/// Bytes no row points at are removed once they are older than this,
/// in the bucket and on a device: an upload whose row never came, bytes
/// written a moment before their row.
pub const UNREFERENCED_DAYS: i64 = 1;
/// The most checksums `list_duplicates` takes in one call.
pub const MAX_DUPLICATES_CHECKED: usize = 500;
/// The most files `search` answers in one call.
pub const MAX_SEARCH_LIMIT: u32 = 500;
/// The waiting files one sync asks about and sends, oldest first; the
/// rest wait for the next, so a URL is used well within its lifetime
/// and the sync lock is not held for thousands of files.
pub const FILES_PER_SYNC: usize = 100;
/// How long a signed upload URL works.
#[cfg(feature = "object-storage")]
const UPLOAD_URL_LIFETIME: std::time::Duration = std::time::Duration::from_secs(10 * 60);
/// How long a signed download URL works.
#[cfg(feature = "object-storage")]
pub const DOWNLOAD_URL_LIFETIME: std::time::Duration = std::time::Duration::from_secs(60 * 60);

/// A file's row, with the sync columns every synced row carries, see
/// `docs/design/offline-sync.md`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct File {
    pub id: Uuid,
    /// Who created it: a record, not an owner, see `files_communities`.
    pub created_by_id: Uuid,
    pub name: String,
    pub content_type: String,
    /// In bytes.
    pub size: u64,
    /// The bytes' SHA-256, lowercase hex.
    pub sha256: String,
    pub inserted_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// The server's version, 0 on a device's file not yet pushed.
    pub version: i64,
    /// When the server last wrote it; none on a device's file not yet pushed.
    pub last_synced_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub deleted_by_id: Option<Uuid>,
}

/// Where a file stands between a device and the server.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileState {
    /// The server has the row and the bytes.
    Synced,
    /// Created on this device, not yet on the server; every sync tries.
    Waiting,
    /// The server will not take it; it stays on this device until the
    /// person tries again or lets it go.
    Refused,
}

/// How a page shows a file's bytes, by its content type. Only these
/// are rendered: an HTML file, say, is a program, and is never loaded
/// as a document. An application adds the types it needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShownAs {
    /// In an `<img>`, animated or not.
    Image,
    /// In a frame, as plain text.
    Text,
    /// In a frame, by the webview's PDF viewer where it has one.
    Pdf,
    /// By its name and size only.
    NotShown,
}

impl ShownAs {
    pub fn of(content_type: &str) -> ShownAs {
        let essence = content_type.split(';').next().unwrap_or_default().trim();
        match essence {
            "text/plain" => ShownAs::Text,
            "application/pdf" => ShownAs::Pdf,
            image if image.starts_with("image/") => ShownAs::Image,
            _ => ShownAs::NotShown,
        }
    }
}

/// A file as the pages show it: with how its bytes are shown, its
/// state, why the last try to send it failed or why it was refused, who
/// created it while the user sees them, and its communities among the
/// user's.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileWithState {
    pub file: File,
    /// None once the creator is no longer in a community of the file's
    /// that the user is in.
    pub creator: Option<User>,
    pub communities: Vec<Community>,
    pub shown_as: ShownAs,
    pub state: FileState,
    pub error: Option<String>,
}

/// What a search of the user's files asks: the files whose names
/// contain `name` in any case, every file for an empty one, of one of
/// `content_types`, any for none, `limit` of them from `offset`, newest
/// first. See `docs/design/files-browsing.md`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileSearch {
    #[serde(default)]
    pub name: String,
    /// Content types without parameters, as `image/png`.
    #[serde(default)]
    pub content_types: Vec<String>,
    #[serde(default)]
    pub offset: u32,
    pub limit: u32,
}

/// What a search answers: how many files match in all, and the page of
/// them asked for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileSearchResults {
    pub total: u64,
    pub files: Vec<FileWithState>,
}

/// The copies a device keeps of files the server holds too: how many,
/// and their size. Removing them loses nothing; each is fetched again
/// the next time it is shown.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilesCache {
    pub count: usize,
    pub bytes: u64,
}

/// A file to create: what the person picked, under the id its bytes
/// are already kept under, in the communities chosen.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Validate)]
pub struct FileNew {
    #[garde(skip)]
    pub id: Uuid,
    #[garde(custom(is_file_name))]
    pub name: String,
    #[garde(custom(is_content_type))]
    pub content_type: String,
    #[garde(custom(is_file_size))]
    pub size: u64,
    #[garde(custom(is_sha256))]
    pub sha256: String,
    /// One at least, checked with the user's memberships.
    #[garde(skip)]
    pub community_ids: Vec<Uuid>,
}

/// A file whose bytes are about to be sent to the bucket. A device
/// names the id it made; the web names none and the server makes one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Validate)]
pub struct FileUploadNew {
    #[garde(skip)]
    #[serde(default)]
    pub id: Option<Uuid>,
    #[garde(custom(is_content_type))]
    pub content_type: String,
    #[garde(custom(is_file_size))]
    pub size: u64,
    #[garde(custom(is_sha256))]
    pub sha256: String,
}

/// The server's answer to `FileUploadNew`: the bytes are already in the
/// bucket, or where to PUT them, with the `Content-Type` and the
/// `x-amz-checksum-sha256` headers the URL was signed for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileUpload {
    pub id: Uuid,
    pub uploaded: bool,
    pub upload_url: Option<String>,
    pub content_type: String,
    /// The bytes' SHA-256 in base64, as the header carries it.
    pub checksum_sha256: String,
}

#[derive(Debug, Error)]
pub enum FileError {
    #[error("file {id} not found")]
    NotFound { id: Uuid },
    #[error("{0}")]
    Invalid(FieldErrors),
    #[error("the file did not arrive; try again")]
    NotUploaded,
    #[error("the change was refused: {0}")]
    Rejected(String),
    #[error("files are not set up on this deployment")]
    NoFileStore,
    #[error("only a device keeps a file's bytes itself")]
    DeviceOnly,
    #[error("{0}")]
    Malformed(String),
    #[error("could not read or write the file on this device: {0}")]
    Io(#[from] std::io::Error),
    #[error("the file is not on this device")]
    NotOnDevice,
    #[error(transparent)]
    Community(#[from] FileCommunityError),
    #[error(transparent)]
    Sync(Box<SyncError>),
    #[cfg(feature = "object-storage")]
    #[error(transparent)]
    Storage(#[from] StorageError),
    #[error(transparent)]
    Record(#[from] crate::app::events::RecordError),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl From<SyncError> for FileError {
    fn from(error: SyncError) -> FileError {
        FileError::Sync(Box::new(error))
    }
}

impl From<serde_json::Error> for FileError {
    fn from(error: serde_json::Error) -> FileError {
        FileError::Malformed(error.to_string())
    }
}

/// The files the user sees, not deleted, newest first, each with its
/// state, creator and communities.
pub async fn list(deployment: &Deployment, user: &User) -> Result<Vec<FileWithState>, FileError> {
    let mut tx = deployment.begin().await?;
    let files = app_files::list(&mut tx, user.id).await?;
    let listed = with_states(&mut tx, deployment.role, user, files).await?;
    tx.commit().await?;
    Ok(listed)
}

/// The files the user sees whose bytes have any of these SHA-256s,
/// newest first, each with its state, creator and communities: what a
/// page offers in place of creating the same files again. Only rows the
/// user already sees are compared, on the server and on a device,
/// offline as well, so it tells nothing of files elsewhere. At most
/// `MAX_DUPLICATES_CHECKED` checksums at once.
pub async fn list_duplicates(
    deployment: &Deployment,
    user: &User,
    sha256s: &[String],
) -> Result<Vec<FileWithState>, FileError> {
    if sha256s.len() > MAX_DUPLICATES_CHECKED {
        return Err(FileError::Invalid(FieldErrors::one(
            "sha256s",
            format!("at most {MAX_DUPLICATES_CHECKED} checksums at once"),
        )));
    }
    for sha256 in sha256s {
        is_sha256(sha256, &()).map_err(|error| {
            FileError::Invalid(FieldErrors::one("sha256s", error.message().to_string()))
        })?;
    }
    if sha256s.is_empty() {
        return Ok(Vec::new());
    }
    let mut tx = deployment.begin().await?;
    let files = app_files::list_with_sha256s(&mut tx, user.id, sha256s).await?;
    let listed = with_states(&mut tx, deployment.role, user, files).await?;
    tx.commit().await?;
    Ok(listed)
}

/// The files the user sees whose names contain the name searched for,
/// of the content types searched for, a page of them newest first, each with its state, creator and
/// communities, and how many match in all. At most `MAX_SEARCH_LIMIT`
/// files at once.
pub async fn search(
    deployment: &Deployment,
    user: &User,
    search: &FileSearch,
) -> Result<FileSearchResults, FileError> {
    if search.limit > MAX_SEARCH_LIMIT {
        return Err(FileError::Invalid(FieldErrors::one(
            "limit",
            format!("at most {MAX_SEARCH_LIMIT} files at once"),
        )));
    }
    let mut tx = deployment.begin().await?;
    let (total, files) = app_files::search(
        &mut tx,
        user.id,
        &search.name,
        &search.content_types,
        i64::from(search.offset),
        i64::from(search.limit),
    )
    .await?;
    let files = with_states(&mut tx, deployment.role, user, files).await?;
    tx.commit().await?;
    Ok(FileSearchResults { total, files })
}

/// One file the user sees, by its id, unless deleted, with its state,
/// creator and communities.
pub async fn get(
    deployment: &Deployment,
    user: &User,
    id: Uuid,
) -> Result<FileWithState, FileError> {
    let mut tx = deployment.begin().await?;
    let file = get_visible(&mut tx, user, id).await?;
    let mut listed = with_states(&mut tx, deployment.role, user, vec![file]).await?;
    tx.commit().await?;
    listed.pop().ok_or(FileError::NotFound { id })
}

/// Writes the row of a file the user creates, whose bytes are already
/// kept under its id: in the bucket on the server, which refuses the row
/// otherwise, in the account's folder on a device. It is in the
/// communities chosen, each one the user is in.
pub async fn create(
    deployment: &Deployment,
    user: &User,
    new: &FileNew,
) -> Result<File, FileError> {
    let new = new.checked()?;
    checked_community_ids(&new.community_ids)?;
    let communities: Vec<FileCommunityNew> = new
        .community_ids
        .iter()
        .map(|community_id| FileCommunityNew {
            id: Uuid::now_v7(),
            community_id: *community_id,
        })
        .collect();
    let event = Event::create(
        TABLE,
        new.id,
        json::object_fields(json!({
            "name": new.name,
            "content_type": new.content_type,
            "size": new.size,
            "sha256": new.sha256,
            "communities": communities,
        })),
        now(),
    );
    apply(deployment, user, &event).await?.file()
}

/// Creates a file on a device: keeps its bytes in the account's folder
/// under a new id, then writes its rows and queues its create, so the
/// file is there with the network off and the next sync sends it. The
/// bytes go first: a crash in between leaves bytes without a row, which
/// the cleanup removes, never a row without its bytes.
pub async fn create_with_bytes(
    deployment: &Deployment,
    user: &User,
    name: &str,
    content_type: &str,
    community_ids: &[Uuid],
    bytes: &[u8],
) -> Result<File, FileError> {
    if deployment.role != Role::Device {
        return Err(FileError::DeviceOnly);
    }
    let owned = bytes.to_vec();
    let sha256 = tokio::task::spawn_blocking(move || sha256::hex(&owned))
        .await
        .map_err(|error| FileError::Malformed(error.to_string()))?;
    let new = FileNew {
        id: Uuid::now_v7(),
        name: name.to_string(),
        content_type: content_type.to_string(),
        size: bytes.len() as u64,
        sha256,
        community_ids: community_ids.to_vec(),
    }
    .checked()?;
    let path = local_path(deployment, new.id)?;
    write_whole(&path, bytes).await?;
    match create(deployment, user, &new).await {
        Ok(file) => Ok(file),
        Err(error) => {
            delete_local(&path).await?;
            Err(error)
        }
    }
}

/// Writes a new name over a file the user sees, and makes its
/// communities among the user's exactly these, one at least; its rows
/// in other communities stay.
pub async fn update(
    deployment: &Deployment,
    user: &User,
    id: Uuid,
    name: &str,
    community_ids: &[Uuid],
) -> Result<File, FileError> {
    let name = checked_name(name)?;
    checked_community_ids(community_ids)?;
    let mut tx = deployment.begin().await?;
    let mut file = get_visible(&mut tx, user, id).await?;
    if file.name != name {
        let event = Event::update(
            TABLE,
            id,
            file.version,
            json::object_fields(json!({ "name": file.name })),
            json::object_fields(json!({ "name": name })),
            now(),
        );
        file = apply_in(&mut tx, deployment.role, user, &event, true)
            .await?
            .file()?;
    }
    files_communities::set_in(&mut tx, deployment.role, user, id, community_ids).await?;
    tx.commit().await?;
    Ok(file)
}

/// Takes a file the user sees out of every community of theirs it is
/// in; the server deletes it when those were its last, and its bytes go
/// after the row, by the sweep on the server and the cleanup on a
/// device. A file on a device that never reached the server, waiting or
/// refused, goes at once and whole, its rows, its events and its bytes,
/// and nothing is pushed.
pub async fn delete(deployment: &Deployment, user: &User, id: Uuid) -> Result<(), FileError> {
    let mut tx = deployment.begin().await?;
    get_visible(&mut tx, user, id).await?;
    if deployment.role == Role::Device && create_of(&mut tx, id).await?.is_some() {
        forget_in(&mut tx, id).await?;
        tx.commit().await?;
        return delete_local_copy(deployment, id).await;
    }
    for row in files_communities::list_visible_of_files(&mut tx, user, &[id]).await? {
        files_communities::delete_in(&mut tx, deployment.role, user, id, row.community_id).await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Applies an event of the user's to its file and answers it: what the
/// server does with a pushed event, and what a local write does on any
/// deployment.
pub async fn apply_event(
    deployment: &Deployment,
    user: &User,
    event: &Event,
) -> Result<Answer, FileError> {
    Ok(apply(deployment, user, event).await?.answer)
}

/// Answers where to PUT a file's bytes, or that the bucket already has
/// them: on the server, which holds the bucket's keys. The file has no
/// row yet, so what holds an upload to its creator is the key's prefix:
/// nobody else writes or claims a file before its row exists.
#[cfg(feature = "object-storage")]
pub async fn create_upload(
    deployment: &Deployment,
    user: &User,
    request: &FileUploadNew,
) -> Result<FileUpload, FileError> {
    if crate::accounts::trial_users::is_trial(deployment, user).await? {
        return Err(FileError::Rejected(
            "a trial has no files; sign up first".to_string(),
        ));
    }
    let storage = bucket(deployment)?;
    let content_type = content_type_or_unknown(&request.content_type);
    let request = FileUploadNew {
        content_type: content_type.clone(),
        ..request.clone()
    };
    FieldErrors::check(&request).map_err(FileError::Invalid)?;
    let id = request.id.unwrap_or_else(Uuid::now_v7);
    let key = storage_key(user.id, id);
    let checksum_sha256 = base64_of_hex(&request.sha256)?;
    if bucket_has(storage, &key, request.size, &checksum_sha256).await? {
        return Ok(FileUpload {
            id,
            uploaded: true,
            upload_url: None,
            content_type,
            checksum_sha256,
        });
    }
    let upload_url = storage
        .upload_url(
            &key,
            &content_type,
            Some(&checksum_sha256),
            UPLOAD_URL_LIFETIME,
        )
        .await?;
    Ok(FileUpload {
        id,
        uploaded: false,
        upload_url: Some(upload_url),
        content_type,
        checksum_sha256,
    })
}

/// A URL a file the user sees is read from for the next hour, signed by
/// the server; as an attachment, the bucket answers it to be saved under
/// the file's name instead of shown.
#[cfg(feature = "object-storage")]
pub async fn content_url(
    deployment: &Deployment,
    user: &User,
    id: Uuid,
    as_attachment: bool,
) -> Result<String, FileError> {
    let storage = bucket(deployment)?;
    let mut tx = deployment.begin().await?;
    let file = get_visible(&mut tx, user, id).await?;
    tx.commit().await?;
    let content_disposition = as_attachment.then(|| attachment_named(&file.name));
    Ok(storage
        .download_url(
            &storage_key(file.created_by_id, file.id),
            DOWNLOAD_URL_LIFETIME,
            content_disposition.as_deref(),
        )
        .await?)
}

/// A file waiting on a device, with what the server said when asked
/// about its bytes: nothing when it could not be asked or would not
/// say, that it has them, or where to send them.
#[derive(Clone, Debug)]
pub struct WaitingFile {
    pub id: Uuid,
    /// The file's create, pending in the queue.
    pub create_id: Uuid,
    pub upload: Option<FileUpload>,
}

/// The first step of a sync on a device: asks the server about the
/// bytes of the first `FILES_PER_SYNC` files whose create is pending,
/// and answers every one, those not asked with nothing. What the device
/// knows of an upload is only this answer, asked again by every sync
/// and kept nowhere. Once the server cannot be reached the rest are not
/// asked, so a device offline waits for one request, not one per file.
pub async fn ask_about_waiting<T: Transport>(
    deployment: &Deployment,
    transport: &T,
    token: Option<&str>,
) -> Result<Vec<WaitingFile>, SyncError> {
    let mut tx = deployment.begin().await?;
    let creates: Vec<StoredEvent> = events::pending(&mut tx)
        .await?
        .into_iter()
        .filter(|stored| stored.event.table == TABLE && stored.event.kind == EventKind::Create)
        .collect();
    tx.commit().await?;

    let mut waiting = Vec::with_capacity(creates.len());
    let mut reachable = true;
    for (index, stored) in creates.into_iter().enumerate() {
        let event = &stored.event;
        let mut file = WaitingFile {
            id: event.row_id,
            create_id: event.id,
            upload: None,
        };
        let request = FileUploadNew {
            id: Some(event.row_id),
            content_type: event.changes["content_type"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            size: event.changes["size"].as_u64().unwrap_or_default(),
            sha256: event.changes["sha256"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
        };
        if reachable && index < FILES_PER_SYNC {
            match transport.files_create_upload(token, request).await {
                Ok(upload) => file.upload = Some(upload),
                // The server will not take this file, whatever is sent
                Err(SyncError::Refused(reason)) => refuse(deployment, event.id, &reason).await?,
                Err(error @ SyncError::Transport(_)) => {
                    reachable = false;
                    note_error(deployment, event.id, &error.to_string()).await?;
                }
                Err(error) => return Err(error),
            }
        }
        waiting.push(file);
    }
    Ok(waiting)
}

/// The rows whose events are held back from the push: the files waiting
/// whose bytes the server does not have, and those it refused, whose
/// later events, a rename say, it would answer by a row it never had,
/// and their rows in communities.
pub async fn held_back(
    deployment: &Deployment,
    waiting: &[WaitingFile],
) -> Result<BTreeSet<Uuid>, SyncError> {
    let mut held_back: BTreeSet<Uuid> = waiting
        .iter()
        .filter(|file| !file.upload.as_ref().is_some_and(|upload| upload.uploaded))
        .map(|file| file.id)
        .collect();
    let mut tx = deployment.begin().await?;
    let refused = events::conflicts(&mut tx)
        .await?
        .into_iter()
        .filter(|stored| stored.event.table == TABLE && stored.event.kind == EventKind::Create)
        .map(|stored| stored.event.row_id);
    held_back.extend(refused);
    let mut rows = BTreeSet::new();
    for stored in events::pending(&mut tx).await? {
        if stored.event.table != app_files_communities::TABLE {
            continue;
        }
        let row = files_communities::get_in(&mut tx, stored.event.row_id)
            .await
            .map_err(FileError::from)?;
        if row.is_some_and(|row| held_back.contains(&row.file_id)) {
            rows.insert(stored.event.row_id);
        }
    }
    held_back.extend(rows);
    tx.commit().await?;
    Ok(held_back)
}

/// The last step of a sync on a device before the files' events are
/// pushed: sends the bytes of each waiting file the server gave a URL
/// for, from the account's folder, and marks it uploaded. How many
/// went. A file whose bytes are gone from the device is refused; one
/// the bucket says no to counts a try and waits for the next sync; once
/// the bucket cannot be reached the rest wait too, and nothing counts.
///
/// This runs inside the sync, which holds the sync lock while bytes go.
/// An application with large files calls it from a task of its own
/// instead, followed by a sync.
pub async fn upload_waiting<T: Transport>(
    deployment: &Deployment,
    transport: &T,
    token: Option<&str>,
    waiting: &mut [WaitingFile],
) -> Result<usize, SyncError> {
    let mut uploaded = 0;
    for file in waiting {
        let Some(upload) = file.upload.as_mut().filter(|upload| !upload.uploaded) else {
            continue;
        };
        let path = local_path(deployment, file.id)?;
        if !tokio::fs::try_exists(&path).await.unwrap_or(false) {
            refuse(deployment, file.create_id, BYTES_GONE).await?;
            continue;
        }
        match transport.files_upload(token, upload, &path).await {
            Ok(()) => {
                upload.uploaded = true;
                uploaded += 1;
            }
            Err(SyncError::Refused(reason)) => {
                let mut tx = deployment.begin().await?;
                events::count_attempt(&mut tx, file.create_id, &reason).await?;
                tx.commit().await?;
            }
            Err(error @ SyncError::Transport(_)) => {
                note_error(deployment, file.create_id, &error.to_string()).await?;
                break;
            }
            Err(error) => return Err(error),
        }
    }
    Ok(uploaded)
}

/// Stops trying a waiting file, with why: kept for the person.
async fn refuse(deployment: &Deployment, create_id: Uuid, reason: &str) -> Result<(), SyncError> {
    let mut tx = deployment.begin().await?;
    events::refuse(&mut tx, create_id, reason).await?;
    tx.commit().await?;
    Ok(())
}

/// Keeps why a waiting file's bytes did not go, without counting a try.
async fn note_error(
    deployment: &Deployment,
    create_id: Uuid,
    reason: &str,
) -> Result<(), SyncError> {
    let mut tx = deployment.begin().await?;
    events::mark_error(&mut tx, create_id, reason).await?;
    tx.commit().await?;
    Ok(())
}

/// Deletes from the bucket every file's bytes that no row points at,
/// deleted rows included, once they are older than `UNREFERENCED_DAYS`:
/// an upload whose push never came, a refused file let go, a row the
/// purge removed. A deleted row keeps its bytes until the purge takes
/// it. On the server, daily after `purge::run` and
/// `avatars::delete_unreferenced`, see `jobs`.
///
/// A device that comes back in the moment between this reading the rows
/// and deleting a file can have its row written just before the bytes
/// go; rare enough to accept, see "A gap accepted" in
/// `docs/design/offline-files-decisions.md`.
#[cfg(feature = "object-storage")]
pub async fn delete_unreferenced(
    deployment: &Deployment,
    now: DateTime<Utc>,
) -> Result<crate::app::avatars::DeleteUnreferencedReport, FileError> {
    let storage = bucket(deployment)?;
    let mut tx = deployment.begin().await?;
    let referenced: BTreeSet<String> = app_files::maintenance::list_ids(&mut tx)
        .await?
        .into_iter()
        .map(|(id, created_by_id, _deleted)| storage_key(created_by_id, id))
        .collect();
    tx.commit().await?;

    let cutoff = now - chrono::Duration::days(UNREFERENCED_DAYS);
    let mut report = crate::app::avatars::DeleteUnreferencedReport::default();
    for file in storage.list(KEY_PREFIX).await? {
        if referenced.contains(&file.key) || file.last_modified > cutoff {
            report.kept += 1;
            continue;
        }
        storage.delete(&file.key).await?;
        report.deleted += 1;
    }
    Ok(report)
}

/// Removes from the account's folder the bytes no live row points at:
/// those of a deleted row at once, and those of no row, or a fetch left
/// half done, once they are older than `UNREFERENCED_DAYS`, since a
/// file being created has its bytes there a moment before its row. That
/// covers a delete, a resync that dropped the rows, and a crash between
/// the bytes and the row. How many went. A deployment with no folder
/// has nothing to remove.
pub async fn delete_unreferenced_on_device(
    deployment: &Deployment,
    now: DateTime<Utc>,
) -> Result<usize, FileError> {
    let FileStore::Folder(folder) = &deployment.file_store else {
        return Ok(0);
    };
    let mut tx = deployment.begin().await?;
    let rows = app_files::maintenance::list_ids(&mut tx).await?;
    tx.commit().await?;

    let cutoff = now - chrono::Duration::days(UNREFERENCED_DAYS);
    let mut removed = 0;
    let mut entries = tokio::fs::read_dir(folder).await?;
    while let Some(entry) = entries.next_entry().await? {
        let row = entry
            .file_name()
            .to_str()
            .and_then(|name| Uuid::parse_str(name).ok())
            .and_then(|id| rows.iter().find(|(row_id, _, _)| *row_id == id));
        let remove = match row {
            Some((_, _, deleted)) => *deleted,
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

/// The copies on this device of the synced files the user sees.
pub async fn cache_get(deployment: &Deployment, user: &User) -> Result<FilesCache, FileError> {
    let mut cache = FilesCache::default();
    for (_path, size) in cached(deployment, user).await? {
        cache.count += 1;
        cache.bytes += size;
    }
    Ok(cache)
}

/// Removes the copies on this device of the synced files the user sees, and
/// answers what went. A waiting or refused file stays: its bytes are
/// nowhere else.
pub async fn cache_delete(deployment: &Deployment, user: &User) -> Result<FilesCache, FileError> {
    let mut removed = FilesCache::default();
    for (path, size) in cached(deployment, user).await? {
        delete_local(&path).await?;
        removed.count += 1;
        removed.bytes += size;
    }
    Ok(removed)
}

/// Where each synced file the user sees is on this device, with its
/// size, for those that are.
async fn cached(deployment: &Deployment, user: &User) -> Result<Vec<(PathBuf, u64)>, FileError> {
    let mut cached = Vec::new();
    for shown in list(deployment, user).await? {
        if shown.state != FileState::Synced {
            continue;
        }
        let path = local_path(deployment, shown.file.id)?;
        match tokio::fs::metadata(&path).await {
            Ok(metadata) => cached.push((path, metadata.len())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(cached)
}

/// The file as it travels on the wire.
pub fn table_row(file: &File) -> Result<TableRow, FileError> {
    Ok(TableRow {
        table: TABLE.to_string(),
        row: serde_json::to_value(file)?,
    })
}

/// Where a file's bytes are in the bucket, under its creator.
pub fn storage_key(created_by_id: Uuid, id: Uuid) -> String {
    format!("{KEY_PREFIX}/{created_by_id}/{id}")
}

/// Writes the server's row over the device's, unless an event of the
/// device's for it is still pending; true when it was written.
pub(crate) async fn apply_server_row(tx: &mut Transaction, row: &Value) -> Result<bool, FileError> {
    let file: File = serde_json::from_value(row.clone())?;
    if events::has_pending(tx, TABLE, file.id).await? {
        return Ok(false);
    }
    app_files::replication::upsert(&mut *tx, AcceptedFile(&file)).await?;
    Ok(true)
}

/// Removes a device's row the server holds under another id.
pub(crate) async fn delete_row(tx: &mut Transaction, id: Uuid) -> Result<(), FileError> {
    Ok(app_files::maintenance::delete(tx, id).await?)
}

/// A page of the rows the user sees that the server wrote after the
/// cursor.
pub(crate) async fn list_synced_after(
    tx: &mut Transaction,
    user: &User,
    after: &Cursor,
    limit: usize,
) -> Result<Vec<File>, FileError> {
    Ok(app_files::replication::list_synced_after(tx, user.id, after, limit).await?)
}

/// The row by its id, deleted or not, whoever sees it.
pub(crate) async fn get_in(tx: &mut Transaction, id: Uuid) -> Result<Option<File>, FileError> {
    Ok(app_files::replication::get(tx, id).await?)
}

/// Where the bytes of a file the user sees are on this device, fetched
/// from the server first when they are not there: the first time a
/// device that did not create the file shows it. Fetched under a temporary name
/// and renamed once whole, so a file at its path is never half there.
/// Working offline, or with the server out of reach, a file not here
/// is `NotOnDevice`.
pub async fn local_copy<T: Transport>(
    deployment: &Deployment,
    transport: &T,
    user: &User,
    id: Uuid,
) -> Result<PathBuf, FileError> {
    let mut tx = deployment.begin().await?;
    let file = get_visible(&mut tx, user, id).await?;
    let offline: bool = device_settings::get(&mut tx, WORK_OFFLINE)
        .await
        .map_err(|error| FileError::Malformed(error.to_string()))?;
    tx.commit().await?;
    let path = local_path(deployment, id)?;
    if tokio::fs::try_exists(&path).await? {
        return Ok(path);
    }
    if offline {
        return Err(FileError::NotOnDevice);
    }
    let (_user, token) = users::signed_in_with_token(deployment)
        .await
        .map_err(|error| FileError::Malformed(error.to_string()))?;
    // A name of its own, since two pages may ask for one file at once
    let partial = path.with_extension(format!("{}.part", Uuid::now_v7().simple()));
    let fetched = match transport.files_download(Some(&token), id, &partial).await {
        Ok(()) => tokio::fs::metadata(&partial)
            .await
            .map(|fetched| fetched.len()),
        Err(SyncError::Transport(_)) => {
            delete_local(&partial).await?;
            return Err(FileError::NotOnDevice);
        }
        Err(error) => {
            delete_local(&partial).await?;
            return Err(error.into());
        }
    };
    if fetched.as_ref().ok() != Some(&file.size) {
        delete_local(&partial).await?;
        return Err(FileError::NotOnDevice);
    }
    tokio::fs::rename(&partial, &path).await?;
    Ok(path)
}

/// Removes the row and every event of a file that never reached the
/// server, on a device, its rows in communities with it; its bytes go
/// once this is committed, see `delete_local_copy`.
pub(crate) async fn forget_in(tx: &mut Transaction, id: Uuid) -> Result<(), FileError> {
    events::delete_of_row(tx, TABLE, id).await?;
    for row in files_communities_of(tx, id).await? {
        events::delete_of_row(tx, app_files_communities::TABLE, row).await?;
    }
    Ok(app_files::maintenance::delete(tx, id).await?)
}

/// The ids of the file's rows in communities with events on this
/// device: those its create wrote, and those written since.
async fn files_communities_of(tx: &mut Transaction, id: Uuid) -> Result<Vec<Uuid>, FileError> {
    let mut rows = Vec::new();
    for stored in events::list_of_table(tx, app_files_communities::TABLE).await? {
        if let Some(row) = files_communities::get_in(tx, stored.event.row_id).await?
            && row.file_id == id
        {
            rows.push(row.id);
        }
    }
    Ok(rows)
}

/// Removes from a device every file its user no longer sees: a
/// community left, a file taken out of theirs or deleted. A file whose
/// create has not reached the server stays: its bytes are nowhere else.
/// Answers the ids that went, whose bytes go once this is committed, see
/// `delete_local_copy`.
pub(crate) async fn prune(tx: &mut Transaction, user: &User) -> Result<Vec<Uuid>, FileError> {
    let mut pruned = Vec::new();
    for id in app_files::maintenance::list_invisible_ids(tx, user.id).await? {
        if create_of(tx, id).await?.is_some() {
            continue;
        }
        app_files::maintenance::delete(tx, id).await?;
        pruned.push(id);
    }
    Ok(pruned)
}

/// Soft-deletes the file at the moment its last row in a community
/// went: on the server it is gone; on a device it is in none of the
/// user's, until a row from the server puts it back.
pub(crate) async fn delete_with_last_community(
    tx: &mut Transaction,
    role: Role,
    id: Uuid,
    deleted_at: Option<DateTime<Utc>>,
    deleted_by_id: Option<Uuid>,
) -> Result<(), FileError> {
    let Some(mut file) = get_in(tx, id).await? else {
        return Ok(());
    };
    if file.deleted_at.is_some() {
        return Ok(());
    }
    file.deleted_at = deleted_at;
    file.deleted_by_id = deleted_by_id;
    file.updated_at = now();
    if role == Role::Server {
        file.version += 1;
        file.last_synced_at = Some(file.updated_at);
    }
    app_files::update(&mut *tx, AcceptedFile(&file)).await?;
    Ok(())
}

/// Removes a file's bytes from this device, if they are there.
pub(crate) async fn delete_local_copy(deployment: &Deployment, id: Uuid) -> Result<(), FileError> {
    delete_local(&local_path(deployment, id)?).await
}

/// Where a file's bytes are on this device, there or not.
pub fn local_path(deployment: &Deployment, id: Uuid) -> Result<PathBuf, FileError> {
    match &deployment.file_store {
        FileStore::Folder(folder) => Ok(folder.join(id.hyphenated().to_string())),
        _ => Err(FileError::NoFileStore),
    }
}

/// Writes the bytes under a temporary name beside the path, then
/// renames, so a file at its path is always whole.
async fn write_whole(path: &Path, bytes: &[u8]) -> Result<(), FileError> {
    let partial = partial_path(path);
    tokio::fs::write(&partial, bytes).await?;
    tokio::fs::rename(&partial, path).await?;
    Ok(())
}

/// The name a file is written under until it is whole.
pub(crate) fn partial_path(path: &Path) -> PathBuf {
    path.with_extension("part")
}

/// Removes the bytes at the path, if any are there.
async fn delete_local(path: &Path) -> Result<(), FileError> {
    match tokio::fs::remove_file(path).await {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error.into()),
        _ => Ok(()),
    }
}

/// The create of a file that has not reached the server, on a device:
/// pending while the file waits, a conflict once the server refused it.
async fn create_of(tx: &mut Transaction, id: Uuid) -> Result<Option<StoredEvent>, FileError> {
    Ok(events::list_of_table(tx, TABLE)
        .await?
        .into_iter()
        .find(|stored| stored.event.row_id == id && stored.event.kind == EventKind::Create))
}

/// The live file, if the user sees it; one they do not see is not
/// found, as if it were not there.
async fn get_visible(tx: &mut Transaction, user: &User, id: Uuid) -> Result<File, FileError> {
    match app_files::get(tx, user.id, id).await? {
        Some(file) => Ok(file),
        _ => Err(FileError::NotFound { id }),
    }
}

/// The files with their states, creators and communities among the
/// user's. On the server every file is synced; on a device a file whose
/// create waits in the queue is waiting, and one whose create the server
/// refused is refused. A creator is shown while they are a live member
/// of one of those communities, as the user sees them on a device.
async fn with_states(
    tx: &mut Transaction,
    role: Role,
    user: &User,
    files: Vec<File>,
) -> Result<Vec<FileWithState>, FileError> {
    let creates: Vec<StoredEvent> = match role {
        Role::Server => Vec::new(),
        Role::Device => events::list_of_table(tx, TABLE)
            .await?
            .into_iter()
            .filter(|stored| stored.event.kind == EventKind::Create)
            .collect(),
    };
    let file_ids: Vec<Uuid> = files.iter().map(|file| file.id).collect();
    let rows = files_communities::list_visible_of_files(tx, user, &file_ids).await?;
    let community_ids: BTreeSet<Uuid> = rows.iter().map(|row| row.community_id).collect();
    let community_ids: Vec<Uuid> = community_ids.into_iter().collect();
    let communities = communities::list_by_ids(tx, &community_ids)
        .await
        .map_err(|error| FileError::Malformed(error.to_string()))?;
    let mut members: Vec<(Uuid, Uuid)> = Vec::new();
    for community_id in &community_ids {
        for membership in app_users_communities::list(
            &mut *tx,
            app_users_communities::Filter::Community(*community_id),
        )
        .await?
        {
            members.push((membership.community_id, membership.user_id));
        }
    }
    let creator_ids: Vec<Uuid> = files.iter().map(|file| file.created_by_id).collect();
    let creators = users::list_by_ids(tx, &creator_ids)
        .await
        .map_err(|error| FileError::Malformed(error.to_string()))?;
    Ok(files
        .into_iter()
        .map(|file| {
            let file_rows: Vec<_> = rows.iter().filter(|row| row.file_id == file.id).collect();
            let creator = creators
                .iter()
                .find(|creator| creator.id == file.created_by_id)
                .filter(|creator| {
                    file_rows
                        .iter()
                        .any(|row| members.contains(&(row.community_id, creator.id)))
                })
                .cloned();
            let communities = file_rows
                .iter()
                .filter_map(|row| {
                    communities
                        .iter()
                        .find(|community| community.id == row.community_id)
                })
                .filter(|community| community.deleted_at.is_none())
                .cloned()
                .collect();
            let create = creates.iter().find(|stored| stored.event.row_id == file.id);
            let (state, error) = match create {
                Some(stored) if stored.status == EventStatus::Pending => {
                    (FileState::Waiting, stored.last_error.clone())
                }
                Some(stored) => (FileState::Refused, stored.last_error.clone()),
                None => (FileState::Synced, None),
            };
            FileWithState {
                shown_as: ShownAs::of(&file.content_type),
                file,
                creator,
                communities,
                state,
                error,
            }
        })
        .collect())
}

/// An event applied: the answer, and the file as it now stands.
pub(crate) struct Applied {
    pub answer: Answer,
    pub file: Option<File>,
}

impl Applied {
    fn file(self) -> Result<File, FileError> {
        match (self.answer.status, self.file) {
            (AnswerStatus::Rejected, _) => {
                let reason = self.answer.reason.unwrap_or_else(|| "rejected".to_string());
                Err(if reason == FILE_NOT_UPLOADED {
                    FileError::NotUploaded
                } else {
                    FileError::Rejected(reason)
                })
            }
            (_, Some(file)) => Ok(file),
            (_, None) => Err(FileError::NotFound { id: self.answer.id }),
        }
    }
}

/// Applies the event in a transaction of its own. The server looks in
/// its bucket first when the event creates a file, outside the
/// transaction, since the bucket is another machine.
async fn apply(deployment: &Deployment, user: &User, event: &Event) -> Result<Applied, FileError> {
    let uploaded = match (deployment.role, event.kind) {
        (Role::Server, EventKind::Create) => is_uploaded(deployment, user, event).await?,
        _ => true,
    };
    let mut tx = deployment.begin().await?;
    let applied = apply_in(&mut tx, deployment.role, user, event, uploaded).await?;
    tx.commit().await?;
    Ok(applied)
}

/// Whether the bucket holds the bytes of the file the event creates, at
/// the size and with the checksum the event says.
async fn is_uploaded(
    deployment: &Deployment,
    user: &User,
    event: &Event,
) -> Result<bool, FileError> {
    let Some(size) = event.changes.get("size").and_then(Value::as_u64) else {
        return Ok(false);
    };
    let Some(sha256) = event.changes.get("sha256").and_then(Value::as_str) else {
        return Ok(false);
    };
    let key = storage_key(user.id, event.row_id);
    bucket_has_bytes(deployment, &key, size, sha256).await
}

#[cfg(feature = "object-storage")]
async fn bucket_has_bytes(
    deployment: &Deployment,
    key: &str,
    size: u64,
    sha256: &str,
) -> Result<bool, FileError> {
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
) -> Result<bool, FileError> {
    Err(FileError::NoFileStore)
}

/// Whether the bucket holds bytes at the key of the size, whose checksum
/// it checked is this base64 SHA-256.
#[cfg(feature = "object-storage")]
async fn bucket_has(
    storage: &Storage,
    key: &str,
    size: u64,
    checksum_sha256: &str,
) -> Result<bool, FileError> {
    if storage.size(key).await? != Some(size) {
        return Ok(false);
    }
    Ok(storage.checksum_sha256(key).await?.as_deref() == Some(checksum_sha256))
}

/// The checksum as the bucket's header carries it, base64, from the
/// row's hex.
#[cfg(feature = "object-storage")]
fn base64_of_hex(sha256: &str) -> Result<String, FileError> {
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
        .ok_or_else(|| FileError::Malformed(format!("{sha256} is not a SHA-256")))?;
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}

/// Applies the event in the transaction: checks the fields it changes
/// and that the user sees the file, or for a create that they are in
/// each of its communities, merges it into the row, writes the row, and
/// then the role's part, the record of the answer on the server or the
/// queue on a device. A create whose bytes are not `uploaded` is
/// answered `file_not_uploaded` and writes nothing.
pub(crate) async fn apply_in(
    tx: &mut Transaction,
    role: Role,
    user: &User,
    event: &Event,
    uploaded: bool,
) -> Result<Applied, FileError> {
    if role == Role::Server
        && let Some(recorded) = events::get(tx, event.id).await?
    {
        let file = get_in(tx, event.row_id).await?;
        let server = file.as_ref().map(table_row).transpose()?;
        return Ok(Applied {
            answer: recorded.answer(server),
            file,
        });
    }

    let allowed = match event.kind {
        EventKind::Create => CREATE_FIELDS,
        EventKind::Update => UPDATE_FIELDS,
    };
    if let Some(reason) = events::not_editable(&event.changes, allowed) {
        let file = get_in(tx, event.row_id).await?;
        return rejected(event, reason, file);
    }

    let now = now();
    match event.kind {
        EventKind::Create => {
            if get_in(tx, event.row_id).await?.is_some() {
                // A file the user does not see is none of their business
                let existing = app_files::get(tx, user.id, event.row_id).await?;
                return rejected(event, "exists", existing);
            }
            let (version, last_synced_at) = match role {
                Role::Server => (1, Some(now)),
                Role::Device => (0, None),
            };
            let blank = File {
                id: event.row_id,
                created_by_id: user.id,
                name: String::new(),
                content_type: String::new(),
                size: 0,
                sha256: String::new(),
                inserted_at: event.row_inserted_at(now),
                updated_at: now,
                version,
                last_synced_at,
                deleted_at: None,
                deleted_by_id: None,
            };
            let file = match blank.with_fields(&event.changes) {
                Ok(file) => file,
                Err(error) => return rejected(event, format!("invalid: {error}"), None),
            };
            let communities: Vec<FileCommunityNew> = match event
                .changes
                .get("communities")
                .map(|value| serde_json::from_value(value.clone()))
                .transpose()
            {
                Ok(communities) => communities.unwrap_or_default(),
                Err(error) => return rejected(event, format!("invalid: {error}"), None),
            };
            if let Some(reason) = files_communities::refusal_of_new(tx, user, &communities).await? {
                return rejected(event, reason, None);
            }
            if !uploaded {
                return rejected(event, FILE_NOT_UPLOADED, None);
            }
            app_files::create(&mut *tx, AcceptedFile(&file)).await?;
            files_communities::create_with_file(tx, user, &file, &communities).await?;
            finish(tx, role, user, event, Conflicts::new(), file).await
        }
        EventKind::Update => {
            let Some(file) = app_files::get(tx, user.id, event.row_id).await? else {
                return rejected(event, "unknown_file", None);
            };
            let (changes, conflicts) = events::merge(
                &file.fields(UPDATE_FIELDS)?,
                file.version,
                event.base_version,
                &event.base,
                &event.changes,
            );
            let file = if changes.is_empty() {
                file
            } else {
                let mut updated = match file.with_fields(&changes) {
                    Ok(updated) => updated,
                    Err(error) => return rejected(event, format!("invalid: {error}"), Some(file)),
                };
                updated.updated_at = now;
                if role == Role::Server {
                    updated.version += 1;
                    updated.last_synced_at = Some(now);
                }
                app_files::update(&mut *tx, AcceptedFile(&updated)).await?;
                updated
            };
            finish(tx, role, user, event, conflicts, file).await
        }
    }
}

/// Answers an applied event and does the role's part.
async fn finish(
    tx: &mut Transaction,
    role: Role,
    user: &User,
    event: &Event,
    conflicts: Conflicts,
    file: File,
) -> Result<Applied, FileError> {
    let answer = Answer::applied(event.id, file.version, conflicts, table_row(&file)?);
    match role {
        Role::Server => events::record(tx, user.id, event, &answer).await?,
        Role::Device => events::queue(tx, user.id, event).await?,
    }
    Ok(Applied {
        answer,
        file: Some(file),
    })
}

fn rejected(
    event: &Event,
    reason: impl Into<String>,
    file: Option<File>,
) -> Result<Applied, FileError> {
    let server = file.as_ref().map(table_row).transpose()?;
    Ok(Applied {
        answer: Answer::rejected(event.id, reason, server),
        file,
    })
}

impl File {
    /// The named fields as they travel on the wire.
    pub(crate) fn fields(&self, names: &[&str]) -> Result<Fields, FileError> {
        let mut all = json::object_fields(serde_json::to_value(self)?);
        all.retain(|name, _| names.contains(&name.as_str()));
        Ok(all)
    }

    /// The file with the fields written over it, checked.
    fn with_fields(&self, changes: &Fields) -> Result<File, FileError> {
        let mut all = json::object_fields(serde_json::to_value(self)?);
        for (name, value) in changes {
            all.insert(name.clone(), value.clone());
        }
        let file: File = serde_json::from_value(Value::Object(all))?;
        let checked = FileNew {
            id: file.id,
            name: file.name.clone(),
            content_type: file.content_type.clone(),
            size: file.size,
            sha256: file.sha256.clone(),
            community_ids: Vec::new(),
        }
        .checked()?;
        Ok(File {
            name: checked.name,
            content_type: checked.content_type,
            ..file
        })
    }
}

impl FileNew {
    /// The file as it is kept: its name trimmed, a missing content type
    /// filled in, and every field within the rules.
    pub fn checked(&self) -> Result<FileNew, FileError> {
        let checked = FileNew {
            id: self.id,
            name: self.name.trim().to_string(),
            content_type: content_type_or_unknown(&self.content_type),
            size: self.size,
            sha256: self.sha256.clone(),
            community_ids: self.community_ids.clone(),
        };
        FieldErrors::check(&checked).map_err(FileError::Invalid)?;
        Ok(checked)
    }
}

/// The server's bucket, where there is one.
#[cfg(feature = "object-storage")]
pub(crate) fn bucket(deployment: &Deployment) -> Result<&Storage, FileError> {
    match &deployment.file_store {
        FileStore::Bucket(storage) => Ok(storage),
        _ => Err(FileError::NoFileStore),
    }
}

/// The name as it is kept: trimmed, not blank, not too long.
fn checked_name(name: &str) -> Result<String, FileError> {
    let name = name.trim();
    is_file_name(name, &()).map_err(|error| {
        FileError::Invalid(FieldErrors::one("name", error.message().to_string()))
    })?;
    Ok(name.to_string())
}

/// A file is in one community at least.
fn checked_community_ids(community_ids: &[Uuid]) -> Result<(), FileError> {
    if community_ids.is_empty() {
        return Err(FileError::Invalid(FieldErrors::one(
            "community_ids",
            "needs a community".to_string(),
        )));
    }
    Ok(())
}

/// The content type a browser gave, or the unknown one when it gave none.
fn content_type_or_unknown(content_type: &str) -> String {
    match content_type.trim() {
        "" => UNKNOWN_CONTENT_TYPE.to_string(),
        content_type => content_type.to_lowercase(),
    }
}

fn is_file_name(value: &str, _context: &()) -> garde::Result {
    if value.trim().is_empty() {
        return Err(garde::Error::new("is blank"));
    }
    if value.chars().count() > MAX_NAME_LENGTH {
        return Err(garde::Error::new("is longer than 255 characters"));
    }
    Ok(())
}

/// A `type/subtype`, with parameters or not, in printable ASCII.
fn is_content_type(value: &str, _context: &()) -> garde::Result {
    let essence = value.split(';').next().unwrap_or_default().trim();
    let well_formed = essence.split_once('/').is_some_and(|(kind, subtype)| {
        !kind.is_empty() && !subtype.is_empty() && !subtype.contains('/')
    });
    let printable = value.chars().all(|c| c.is_ascii_graphic() || c == ' ');
    if !well_formed || !printable || value.len() > MAX_NAME_LENGTH {
        return Err(garde::Error::new("is not a content type"));
    }
    Ok(())
}

/// A SHA-256 as lowercase hex.
fn is_sha256(value: &str, _context: &()) -> garde::Result {
    let hex = value
        .chars()
        .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c));
    if value.len() != 64 || !hex {
        return Err(garde::Error::new("is not a SHA-256"));
    }
    Ok(())
}

fn is_file_size(value: &u64, _context: &()) -> garde::Result {
    if *value == 0 {
        return Err(garde::Error::new("is empty"));
    }
    if *value > MAX_BYTES {
        return Err(garde::Error::new("is larger than 25 MB"));
    }
    Ok(())
}

/// The `Content-Disposition` that has a browser save a file under the
/// name, which travels percent-encoded so any name is safe in a header.
#[cfg(feature = "object-storage")]
fn attachment_named(name: &str) -> String {
    let mut encoded = String::new();
    for byte in name.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_') {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    format!("attachment; filename*=UTF-8''{encoded}")
}

#[cfg(all(test, feature = "object-storage"))]
pub(crate) mod tests {
    use super::*;
    use crate::accounts::Session;
    use crate::app::conflicts::{self, ConflictError, Resolve};
    use crate::app::sync::tests::user_with_token;
    use crate::app::sync::{
        self, BackfillRequest, InProcess, Page, PullRequest, PushRequest, PushResponse,
    };
    use crate::app::{pending_changes, posts, users};
    use crate::database::{Database, with_each_database};

    /// The user's own community on the deployment, made the first time.
    pub(crate) async fn personal(deployment: &Deployment, user: &User) -> Uuid {
        let mut tx = deployment.begin().await.expect("a transaction");
        let mine =
            app_users_communities::list(&mut tx, app_users_communities::Filter::User(user.id))
                .await
                .expect("the memberships");
        tx.commit().await.expect("committed");
        match mine.first() {
            Some(membership) => membership.community_id,
            None => {
                communities::create(deployment, user, "Personal")
                    .await
                    .expect("a community")
                    .id
            }
        }
    }

    /// Creates a file on a device, in the user's own community there.
    pub(crate) async fn with_bytes(
        deployment: &Deployment,
        user: &User,
        name: &str,
        content_type: &str,
        bytes: &[u8],
    ) -> Result<File, FileError> {
        let community_id = personal(deployment, user).await;
        create_with_bytes(deployment, user, name, content_type, &[community_id], bytes).await
    }

    /// Renames a file, leaving its communities as they are.
    pub(crate) async fn rename(
        deployment: &Deployment,
        user: &User,
        id: Uuid,
        name: &str,
    ) -> Result<File, FileError> {
        let community_ids: Vec<Uuid> = match get(deployment, user, id).await {
            Ok(shown) => shown
                .communities
                .iter()
                .map(|community| community.id)
                .collect(),
            Err(_) => vec![Uuid::now_v7()],
        };
        update(deployment, user, id, name, &community_ids).await
    }

    /// A server with a bucket in memory.
    pub(crate) fn server(database: Database) -> (Deployment, Storage) {
        let storage = Storage::memory();
        let deployment = Deployment::new(database, Role::Server)
            .with_file_store(FileStore::Bucket(storage.clone()));
        (deployment, storage)
    }

    pub(crate) fn new_file(name: &str, bytes: &[u8], community_id: Uuid) -> FileNew {
        FileNew {
            id: Uuid::now_v7(),
            name: name.to_string(),
            content_type: "text/plain".to_string(),
            size: bytes.len() as u64,
            sha256: sha256::hex(bytes),
            community_ids: vec![community_id],
        }
    }

    /// A file the user created on the server, in their own community,
    /// its bytes put in the bucket as an upload would have.
    pub(crate) async fn uploaded(
        deployment: &Deployment,
        storage: &Storage,
        user: &User,
        name: &str,
        bytes: &[u8],
    ) -> File {
        let new = new_file(name, bytes, personal(deployment, user).await);
        storage
            .put(&storage_key(user.id, new.id), bytes.to_vec())
            .await
            .expect("the upload");
        create(deployment, user, &new).await.expect("a file")
    }

    /// A device the user is signed in on, with a folder for its files'
    /// bytes, which goes when the test drops it.
    pub(crate) async fn device_with_folder(
        user: &User,
        token: &str,
    ) -> (Deployment, tempfile::TempDir) {
        let folder = tempfile::tempdir().expect("a folder");
        let deployment = users::tests::device_of(user, token)
            .await
            .with_file_store(FileStore::Folder(folder.path().to_path_buf()));
        (deployment, folder)
    }

    #[tokio::test]
    async fn files_records_enforce_owner_and_keep_cleanup_references_for_tombstones() {
        with_each_database(|database| async move {
            let (deployment, storage) = server(database);
            let alice = users::create(&deployment, "alice@example.com", "Alice")
                .await
                .expect("user");
            let bob = users::create(&deployment, "bob@example.com", "Bob")
                .await
                .expect("user");
            let file = uploaded(&deployment, &storage, &alice, "notes.txt", b"x").await;
            let mut tx = deployment.begin().await.expect("transaction");
            assert!(
                app_files::get(&mut tx, alice.id, file.id)
                    .await
                    .expect("owner")
                    .is_some()
            );
            assert!(
                app_files::get(&mut tx, bob.id, file.id)
                    .await
                    .expect("other user")
                    .is_none()
            );
            tx.commit().await.expect("commit");
            delete(&deployment, &alice, file.id).await.expect("delete");
            let mut tx = deployment.begin().await.expect("transaction");
            assert!(
                app_files::get(&mut tx, alice.id, file.id)
                    .await
                    .expect("ordinary read")
                    .is_none()
            );
            assert!(
                app_files::list(&mut tx, alice.id)
                    .await
                    .expect("list")
                    .is_empty()
            );
            assert!(
                app_files::replication::get(&mut tx, file.id)
                    .await
                    .expect("read")
                    .expect("tombstone")
                    .deleted_at
                    .is_some()
            );
            assert!(
                app_files::maintenance::list_ids(&mut tx)
                    .await
                    .expect("references")
                    .contains(&(file.id, alice.id, true))
            );
        })
        .await;
    }

    #[tokio::test]
    async fn files_apply_in_rolls_back_row_and_event_together() {
        with_each_database(|database| async move {
            let (deployment, storage) = server(database);
            let alice = users::create(&deployment, "alice@example.com", "Alice")
                .await
                .expect("user");
            let file = uploaded(&deployment, &storage, &alice, "before.txt", b"x").await;
            let event = Event::update(
                TABLE,
                file.id,
                file.version,
                json::object_fields(json!({"name": file.name})),
                json::object_fields(json!({"name": "after.txt"})),
                now(),
            );
            let mut tx = deployment.begin().await.expect("transaction");
            let applied = apply_in(&mut tx, Role::Server, &alice, &event, true)
                .await
                .expect("applied");
            assert_eq!(applied.file.expect("file").name, "after.txt");
            assert!(
                events::get(&mut tx, event.id)
                    .await
                    .expect("event")
                    .is_some()
            );
            drop(tx);
            assert_eq!(
                get(&deployment, &alice, file.id)
                    .await
                    .expect("restored")
                    .file,
                file
            );
            let mut tx = deployment.begin().await.expect("transaction");
            assert!(
                events::get(&mut tx, event.id)
                    .await
                    .expect("event")
                    .is_none()
            );
        })
        .await;
    }

    #[tokio::test]
    async fn files_create_with_bytes_on_a_device_keeps_them_and_waits() {
        let alice = users::tests::alice();
        let (device, folder) = device_with_folder(&alice, "a token").await;

        let file = with_bytes(&device, &alice, " notes.txt ", "text/plain", b"some notes")
            .await
            .expect("a file");
        assert_eq!(file.name, "notes.txt");
        assert_eq!(file.size, 10);
        assert_eq!(file.version, 0);
        let path = local_path(&device, file.id).expect("a path");
        assert_eq!(std::fs::read(&path).expect("the bytes"), b"some notes");
        let listed = list(&device, &alice).await.expect("the files");
        assert_eq!(listed[0].file, file);
        assert_eq!(listed[0].state, FileState::Waiting);

        let error = with_bytes(&device, &alice, "empty.txt", "text/plain", b"")
            .await
            .expect_err("no bytes");
        assert!(matches!(error, FileError::Invalid(_)));
        assert_eq!(
            std::fs::read_dir(folder.path())
                .expect("the folder")
                .count(),
            1,
            "a file outside the rules leaves no bytes"
        );
    }

    #[tokio::test]
    async fn files_delete_of_a_waiting_file_on_a_device_leaves_nothing() {
        let alice = users::tests::alice();
        let (device, folder) = device_with_folder(&alice, "a token").await;
        let file = with_bytes(&device, &alice, "notes.txt", "text/plain", b"x")
            .await
            .expect("a file");
        rename(&device, &alice, file.id, "renamed.txt")
            .await
            .expect("renamed while waiting");

        delete(&device, &alice, file.id).await.expect("deleted");
        assert!(list(&device, &alice).await.expect("the files").is_empty());
        let mut tx = device.begin().await.expect("a transaction");
        assert!(get_in(&mut tx, file.id).await.expect("looked").is_none());
        assert!(
            events::list_of_table(&mut tx, TABLE)
                .await
                .expect("the events")
                .is_empty(),
            "nothing is left to push"
        );
        assert_eq!(
            std::fs::read_dir(folder.path())
                .expect("the folder")
                .count(),
            0
        );
    }

    /// The server in this process, with the faults a test asks for.
    struct Faulty<'a> {
        server: InProcess<'a>,
        /// How many pushes carrying a file's event are lost on the way.
        file_pushes_lost: std::sync::Mutex<u32>,
        /// Answers that the bucket has every file's bytes, asked or not.
        says_uploaded: bool,
        /// Cannot be reached about files, as with the bucket down.
        down_for_files: bool,
        /// Will not take any file, and says why.
        refuses_files: Option<&'static str>,
    }

    impl<'a> Faulty<'a> {
        fn new(server: &'a Deployment) -> Faulty<'a> {
            Faulty {
                server: InProcess { server },
                file_pushes_lost: std::sync::Mutex::new(0),
                says_uploaded: false,
                down_for_files: false,
                refuses_files: None,
            }
        }
    }

    impl Transport for Faulty<'_> {
        async fn push(
            &self,
            token: Option<&str>,
            request: PushRequest,
        ) -> Result<PushResponse, SyncError> {
            let carries_a_file = request.events.iter().any(|event| event.table == TABLE);
            {
                let mut lost = self.file_pushes_lost.lock().expect("the lock");
                if carries_a_file && *lost > 0 {
                    *lost -= 1;
                    return Err(SyncError::Transport("the connection dropped".to_string()));
                }
            }
            self.server.push(token, request).await
        }

        async fn pull(&self, token: Option<&str>, request: PullRequest) -> Result<Page, SyncError> {
            self.server.pull(token, request).await
        }

        async fn backfill(
            &self,
            token: Option<&str>,
            request: BackfillRequest,
        ) -> Result<Page, SyncError> {
            self.server.backfill(token, request).await
        }

        async fn sign_in_with_device_code(
            &self,
            code: &str,
            verifier: &str,
        ) -> Result<Session, SyncError> {
            self.server.sign_in_with_device_code(code, verifier).await
        }

        async fn sign_out(&self, token: &str) -> Result<(), SyncError> {
            self.server.sign_out(token).await
        }

        async fn files_create_upload(
            &self,
            token: Option<&str>,
            request: FileUploadNew,
        ) -> Result<FileUpload, SyncError> {
            if self.down_for_files {
                return Err(SyncError::Transport("502 from the server".to_string()));
            }
            if let Some(reason) = self.refuses_files {
                return Err(SyncError::Refused(reason.to_string()));
            }
            let upload = self.server.files_create_upload(token, request).await?;
            Ok(FileUpload {
                uploaded: upload.uploaded || self.says_uploaded,
                ..upload
            })
        }

        async fn files_upload(
            &self,
            token: Option<&str>,
            upload: &FileUpload,
            path: &Path,
        ) -> Result<(), SyncError> {
            self.server.files_upload(token, upload, path).await
        }

        async fn files_download(
            &self,
            token: Option<&str>,
            id: Uuid,
            path: &Path,
        ) -> Result<(), SyncError> {
            self.server.files_download(token, id, path).await
        }
    }

    async fn state_of(
        deployment: &Deployment,
        user: &User,
        id: Uuid,
    ) -> (FileState, Option<String>) {
        let shown = get(deployment, user, id).await.expect("the file");
        (shown.state, shown.error)
    }

    async fn in_bucket(storage: &Storage, user: &User, id: Uuid) -> Option<u64> {
        storage
            .size(&storage_key(user.id, id))
            .await
            .expect("looked")
    }

    #[tokio::test]
    async fn files_added_offline_reach_the_bucket_the_server_and_another_device() {
        with_each_database(|database| async move {
            let (server, storage) = server(database);
            let (alice, token) = user_with_token(&server, "alice@example.com").await;
            let (a, _a_folder) = device_with_folder(&alice, &token).await;
            let (b, _b_folder) = device_with_folder(&alice, &token).await;
            let file = with_bytes(&a, &alice, "notes.txt", "text/plain", b"some notes")
                .await
                .expect("a file");
            posts::create(&a, &alice, "a post beside it", &[])
                .await
                .expect("a post");
            assert_eq!(in_bucket(&storage, &alice, file.id).await, None);

            let report = sync::run(&a, &InProcess { server: &server })
                .await
                .expect("a sync");
            assert_eq!(report.uploaded, 1);
            assert_eq!(
                report.pushed, 4,
                "the community, its membership and the post, then the file once its bytes went"
            );
            assert_eq!(report.pending, 0);
            assert_eq!(report.push_error, None);
            assert_eq!(in_bucket(&storage, &alice, file.id).await, Some(10));
            let on_server = get(&server, &alice, file.id).await.expect("on the server");
            assert_eq!(on_server.file.name, "notes.txt");
            assert_eq!(on_server.file.version, 1);
            let on_a = get(&a, &alice, file.id).await.expect("on a");
            assert_eq!(on_a.state, FileState::Synced);
            assert_eq!(on_a.file, on_server.file, "the server's row came back");
            assert!(
                local_path(&a, file.id).expect("a path").exists(),
                "the device that added it keeps its copy"
            );

            sync::run(&b, &InProcess { server: &server })
                .await
                .expect("a sync");
            let on_b = get(&b, &alice, file.id).await.expect("on b");
            assert_eq!(on_b.file, on_server.file);
            assert_eq!(on_b.state, FileState::Synced);
        })
        .await;
    }

    #[tokio::test]
    async fn files_local_copy_fetches_once_on_another_device_then_reads_with_the_network_off() {
        with_each_database(|database| async move {
            let (server, storage) = server(database);
            let (alice, token) = user_with_token(&server, "alice@example.com").await;
            let (bob, bobs_token) = user_with_token(&server, "bob@example.com").await;
            let (a, _a_folder) = device_with_folder(&alice, &token).await;
            let (b, b_folder) = device_with_folder(&alice, &token).await;
            let (bobs, _bobs_folder) = device_with_folder(&bob, &bobs_token).await;
            let transport = InProcess { server: &server };
            let file = with_bytes(&a, &alice, "notes.txt", "text/plain", b"some notes")
                .await
                .expect("a file");
            sync::run(&a, &transport).await.expect("a sync");
            sync::run(&b, &transport).await.expect("a sync");
            assert!(!local_path(&b, file.id).expect("a path").exists());

            let path = local_copy(&b, &transport, &alice, file.id)
                .await
                .expect("fetched");
            assert_eq!(std::fs::read(&path).expect("the bytes"), b"some notes");
            assert_eq!(
                std::fs::read_dir(b_folder.path())
                    .expect("the folder")
                    .count(),
                1,
                "nothing half fetched is left"
            );

            storage
                .delete(&storage_key(alice.id, file.id))
                .await
                .expect("the bucket out of reach, in effect");
            let again = local_copy(&b, &transport, &alice, file.id)
                .await
                .expect("read from the device");
            assert_eq!(again, path);

            let error = local_copy(&bobs, &transport, &bob, file.id)
                .await
                .expect_err("not bob's");
            assert!(matches!(error, FileError::NotFound { .. }));
        })
        .await;
    }

    #[tokio::test]
    async fn files_cache_delete_removes_synced_copies_and_leaves_waiting_files() {
        with_each_database(|database| async move {
            let (server, _storage) = server(database);
            let (alice, token) = user_with_token(&server, "alice@example.com").await;
            let (device, _folder) = device_with_folder(&alice, &token).await;
            let transport = InProcess { server: &server };
            let synced = with_bytes(&device, &alice, "notes.txt", "text/plain", b"notes")
                .await
                .expect("a file");
            sync::run(&device, &transport).await.expect("a sync");
            let waiting = with_bytes(&device, &alice, "more.txt", "text/plain", b"more")
                .await
                .expect("a file");

            let cache = cache_get(&device, &alice).await.expect("the cache");
            assert_eq!(cache, FilesCache { count: 1, bytes: 5 });
            let removed = cache_delete(&device, &alice).await.expect("removed");
            assert_eq!(removed, cache);
            assert!(!local_path(&device, synced.id).expect("a path").exists());
            assert!(
                local_path(&device, waiting.id).expect("a path").exists(),
                "a waiting file is the only copy"
            );
            assert_eq!(
                cache_get(&device, &alice).await.expect("the cache"),
                FilesCache::default()
            );

            let path = local_copy(&device, &transport, &alice, synced.id)
                .await
                .expect("fetched again");
            assert_eq!(std::fs::read(&path).expect("the bytes"), b"notes");
        })
        .await;
    }

    #[tokio::test]
    async fn files_local_copy_working_offline_fetches_nothing() {
        with_each_database(|database| async move {
            let (server, _storage) = server(database);
            let (alice, token) = user_with_token(&server, "alice@example.com").await;
            let (a, _a_folder) = device_with_folder(&alice, &token).await;
            let (b, b_folder) = device_with_folder(&alice, &token).await;
            let transport = InProcess { server: &server };
            let file = with_bytes(&a, &alice, "notes.txt", "text/plain", b"some notes")
                .await
                .expect("a file");
            sync::run(&a, &transport).await.expect("a sync");
            sync::run(&b, &transport).await.expect("a sync");
            sync::set_offline(&b, true).await.expect("offline");

            let error = local_copy(&b, &transport, &alice, file.id)
                .await
                .expect_err("not fetched");
            assert!(matches!(error, FileError::NotOnDevice));
            assert_eq!(
                std::fs::read_dir(b_folder.path())
                    .expect("the folder")
                    .count(),
                0
            );
            assert!(
                local_copy(&a, &transport, &alice, file.id).await.is_ok(),
                "the device that added it has it"
            );
        })
        .await;
    }

    #[tokio::test]
    async fn files_a_failed_upload_holds_back_only_its_own_events() {
        with_each_database(|database| async move {
            let (server, storage) = server(database);
            let (alice, token) = user_with_token(&server, "alice@example.com").await;
            let (a, _folder) = device_with_folder(&alice, &token).await;
            let lost = with_bytes(&a, &alice, "lost.txt", "text/plain", b"lost")
                .await
                .expect("a file");
            let kept = with_bytes(&a, &alice, "kept.txt", "text/plain", b"kept")
                .await
                .expect("a file");
            rename(&a, &alice, lost.id, "renamed.txt")
                .await
                .expect("renamed while waiting");
            posts::create(&a, &alice, "a post", &[])
                .await
                .expect("a post");
            std::fs::remove_file(local_path(&a, lost.id).expect("a path")).expect("its bytes gone");

            let report = sync::run(&a, &InProcess { server: &server })
                .await
                .expect("a sync");
            assert_eq!(report.uploaded, 1);
            assert_eq!(
                report.pushed, 4,
                "the community, its membership, the post and the other file"
            );
            assert_eq!(report.pending, 1, "its rename, held back with it");
            assert_eq!(in_bucket(&storage, &alice, kept.id).await, Some(4));
            assert_eq!(in_bucket(&storage, &alice, lost.id).await, None);
            let on_server: Vec<String> = list(&server, &alice)
                .await
                .expect("the files")
                .into_iter()
                .map(|listed| listed.file.name)
                .collect();
            assert_eq!(on_server, ["kept.txt"]);
            let (state, error) = state_of(&a, &alice, lost.id).await;
            assert_eq!(state, FileState::Refused, "there is nothing to send");
            assert_eq!(error.as_deref(), Some(BYTES_GONE));
            assert_eq!(state_of(&a, &alice, kept.id).await.0, FileState::Synced);

            let report = sync::run(&a, &InProcess { server: &server })
                .await
                .expect("a sync");
            assert_eq!(
                report.pushed, 0,
                "its rename is not sent for a row the server never had"
            );
            assert_eq!(
                get(&a, &alice, lost.id).await.expect("kept").file.name,
                "renamed.txt"
            );
        })
        .await;
    }

    #[tokio::test]
    async fn files_a_sync_sends_files_per_sync_and_leaves_the_rest_for_the_next() {
        with_each_database(|database| async move {
            let (server, storage) = server(database);
            let (alice, token) = user_with_token(&server, "alice@example.com").await;
            let (a, _folder) = device_with_folder(&alice, &token).await;
            let mut created = Vec::new();
            for number in 0..=FILES_PER_SYNC {
                let name = format!("{number}.txt");
                let file = with_bytes(&a, &alice, &name, "text/plain", name.as_bytes())
                    .await
                    .expect("a file");
                created.push(file);
            }
            let last = created.last().expect("the last file");

            let report = sync::run(&a, &InProcess { server: &server })
                .await
                .expect("a sync");
            assert_eq!(report.uploaded, FILES_PER_SYNC);
            assert_eq!(report.files_left, 1);
            assert!(report.more_files_to_send());
            assert_eq!(report.pending, 1);
            assert_eq!(in_bucket(&storage, &alice, last.id).await, None);
            assert_eq!(
                state_of(&a, &alice, last.id).await,
                (FileState::Waiting, None),
                "held back, not pushed without its bytes"
            );

            let report = sync::run(&a, &InProcess { server: &server })
                .await
                .expect("a sync");
            assert_eq!(report.uploaded, 1);
            assert_eq!(report.files_left, 0);
            assert!(!report.more_files_to_send());
            assert_eq!(report.pending, 0);
            assert_eq!(state_of(&a, &alice, last.id).await.0, FileState::Synced);
        })
        .await;
    }

    #[tokio::test]
    async fn files_a_sync_cut_off_after_the_upload_is_finished_by_the_next() {
        with_each_database(|database| async move {
            let (server, storage) = server(database);
            let (alice, token) = user_with_token(&server, "alice@example.com").await;
            let (a, _folder) = device_with_folder(&alice, &token).await;
            let file = with_bytes(&a, &alice, "notes.txt", "text/plain", b"some notes")
                .await
                .expect("a file");
            let transport = Faulty::new(&server);
            *transport.file_pushes_lost.lock().expect("the lock") = 1;

            let report = sync::run(&a, &transport).await.expect("a sync");
            assert_eq!(report.uploaded, 1);
            assert!(report.push_error.is_some());
            assert_eq!(in_bucket(&storage, &alice, file.id).await, Some(10));
            assert_eq!(state_of(&a, &alice, file.id).await.0, FileState::Waiting);

            let report = sync::run(&a, &transport).await.expect("a sync");
            assert_eq!(report.uploaded, 0, "the server said it has the bytes");
            assert_eq!(report.pushed, 1);
            assert_eq!(state_of(&a, &alice, file.id).await.0, FileState::Synced);
            assert!(get(&server, &alice, file.id).await.is_ok());
        })
        .await;
    }

    #[tokio::test]
    async fn files_bytes_gone_from_the_bucket_before_the_push_are_sent_again() {
        with_each_database(|database| async move {
            let (server, storage) = server(database);
            let (alice, token) = user_with_token(&server, "alice@example.com").await;
            let (a, _folder) = device_with_folder(&alice, &token).await;
            let file = with_bytes(&a, &alice, "notes.txt", "text/plain", b"some notes")
                .await
                .expect("a file");
            let transport = Faulty::new(&server);
            *transport.file_pushes_lost.lock().expect("the lock") = 1;
            sync::run(&a, &transport).await.expect("a sync cut off");
            storage
                .delete(&storage_key(alice.id, file.id))
                .await
                .expect("swept, as a file no row points at is after a day");

            let report = sync::run(&a, &transport).await.expect("a sync");
            assert_eq!(report.uploaded, 1);
            assert_eq!(in_bucket(&storage, &alice, file.id).await, Some(10));
            assert_eq!(state_of(&a, &alice, file.id).await.0, FileState::Synced);
        })
        .await;
    }

    #[tokio::test]
    async fn files_a_create_pushed_without_its_bytes_stays_waiting_and_is_tried_again() {
        with_each_database(|database| async move {
            let (server, _storage) = server(database);
            let (alice, token) = user_with_token(&server, "alice@example.com").await;
            let (a, _folder) = device_with_folder(&alice, &token).await;
            let file = with_bytes(&a, &alice, "notes.txt", "text/plain", b"some notes")
                .await
                .expect("a file");
            let wrong = Faulty {
                says_uploaded: true,
                ..Faulty::new(&server)
            };

            let report = sync::run(&a, &wrong).await.expect("a sync");
            assert_eq!(report.rejected, 1);
            assert_eq!(report.pending, 1);
            let (state, error) = state_of(&a, &alice, file.id).await;
            assert_eq!(state, FileState::Waiting, "the file is not dropped");
            assert_eq!(error.as_deref(), Some(FILE_NOT_UPLOADED));
            assert!(get(&server, &alice, file.id).await.is_err());

            sync::run(&a, &InProcess { server: &server })
                .await
                .expect("a sync");
            assert_eq!(state_of(&a, &alice, file.id).await.0, FileState::Synced);
        })
        .await;
    }

    #[tokio::test]
    async fn files_the_server_saying_no_thirty_times_refuses_a_file_which_is_then_tried_again() {
        with_each_database(|database| async move {
            let (server, _storage) = server(database);
            let (alice, token) = user_with_token(&server, "alice@example.com").await;
            let (a, _folder) = device_with_folder(&alice, &token).await;
            let file = with_bytes(&a, &alice, "notes.txt", "text/plain", b"some notes")
                .await
                .expect("a file");
            let wrong = Faulty {
                says_uploaded: true,
                ..Faulty::new(&server)
            };

            for _ in 1..events::MAX_ATTEMPTS {
                let report = sync::run(&a, &wrong).await.expect("a sync");
                assert_eq!(report.conflicts, 0);
            }
            assert_eq!(state_of(&a, &alice, file.id).await.0, FileState::Waiting);
            let report = sync::run(&a, &wrong).await.expect("the thirtieth");
            assert_eq!(report.conflicts, 1);
            assert_eq!(report.pending, 0);
            let (state, error) = state_of(&a, &alice, file.id).await;
            assert_eq!(state, FileState::Refused);
            assert_eq!(error.as_deref(), Some("tried 30 times: file_not_uploaded"));
            assert!(
                local_path(&a, file.id).expect("a path").exists(),
                "not lost"
            );

            let report = sync::run(&a, &wrong).await.expect("a sync");
            assert_eq!(
                report.rejected, 0,
                "a refused file is not sent again on its own"
            );
            let listed = conflicts::list(&a).await.expect("the conflicts");
            assert_eq!(listed.len(), 1);
            assert_eq!(listed[0].about.as_deref(), Some("notes.txt"));
            assert_eq!(listed[0].resolve, Resolve::TryAgain);
            assert_eq!(listed[0].reason, error);
            let error = conflicts::reapply(&a, &alice, listed[0].id)
                .await
                .expect_err("a create is tried again");
            assert!(matches!(error, ConflictError::NotReapplied));

            conflicts::try_again(&a, listed[0].id)
                .await
                .expect("queued again");
            assert_eq!(
                state_of(&a, &alice, file.id).await,
                (FileState::Waiting, None)
            );
            sync::run(&a, &wrong).await.expect("a sync");
            let mut tx = a.begin().await.expect("a transaction");
            let create = create_of(&mut tx, file.id)
                .await
                .expect("looked")
                .expect("pending");
            assert_eq!(create.attempts, 1, "counted from none again");
            tx.commit().await.expect("committed");

            sync::run(&a, &InProcess { server: &server })
                .await
                .expect("a sync");
            assert_eq!(state_of(&a, &alice, file.id).await.0, FileState::Synced);
            assert!(conflicts::list(&a).await.expect("none").is_empty());
        })
        .await;
    }

    #[tokio::test]
    async fn files_a_server_or_bucket_that_is_failing_counts_no_try_however_long() {
        with_each_database(|database| async move {
            let (server, _storage) = server(database);
            let (alice, token) = user_with_token(&server, "alice@example.com").await;
            let (a, _folder) = device_with_folder(&alice, &token).await;
            let file = with_bytes(&a, &alice, "notes.txt", "text/plain", b"some notes")
                .await
                .expect("a file");
            posts::create(&a, &alice, "a post", &[])
                .await
                .expect("a post");
            let down = Faulty {
                down_for_files: true,
                ..Faulty::new(&server)
            };

            for _ in 0..events::MAX_ATTEMPTS + 5 {
                sync::run(&a, &down).await.expect("a sync");
            }
            let (state, error) = state_of(&a, &alice, file.id).await;
            assert_eq!(state, FileState::Waiting);
            assert!(error.is_some(), "why is shown under the file");
            let mut tx = a.begin().await.expect("a transaction");
            let create = create_of(&mut tx, file.id)
                .await
                .expect("looked")
                .expect("pending");
            assert_eq!(create.attempts, 0);
            tx.commit().await.expect("committed");
            assert_eq!(
                posts::list(&server, &alice).await.expect("the posts").len(),
                1,
                "the post went all the same"
            );

            sync::run(&a, &InProcess { server: &server })
                .await
                .expect("a sync");
            assert_eq!(state_of(&a, &alice, file.id).await.0, FileState::Synced);
        })
        .await;
    }

    #[tokio::test]
    async fn files_a_file_the_server_will_not_take_is_kept_until_let_go() {
        with_each_database(|database| async move {
            let (server, _storage) = server(database);
            let (alice, token) = user_with_token(&server, "alice@example.com").await;
            let (a, folder) = device_with_folder(&alice, &token).await;
            let file = with_bytes(&a, &alice, "notes.txt", "text/plain", b"some notes")
                .await
                .expect("a file");
            let strict = Faulty {
                refuses_files: Some("size: is larger than 1 KB"),
                ..Faulty::new(&server)
            };

            let report = sync::run(&a, &strict).await.expect("a sync");
            assert_eq!(report.pending, 0);
            let (state, error) = state_of(&a, &alice, file.id).await;
            assert_eq!(
                state,
                FileState::Refused,
                "at once: trying again changes nothing"
            );
            assert_eq!(error.as_deref(), Some("size: is larger than 1 KB"));
            assert_eq!(sync::status(&a).await.expect("the status").conflicts, 1);
            rename(&a, &alice, file.id, "renamed.txt")
                .await
                .expect("renamed while refused");
            sync::run(&a, &strict).await.expect("a sync");
            assert_eq!(
                get(&a, &alice, file.id)
                    .await
                    .expect("still here")
                    .file
                    .name,
                "renamed.txt"
            );

            let listed = conflicts::list(&a).await.expect("the conflicts");
            conflicts::let_go(&a, listed[0].id).await.expect("let go");
            assert!(list(&a, &alice).await.expect("the files").is_empty());
            assert_eq!(sync::status(&a).await.expect("the status").pending, 0);
            assert_eq!(
                std::fs::read_dir(folder.path())
                    .expect("the folder")
                    .count(),
                0
            );
        })
        .await;
    }

    #[tokio::test]
    async fn files_a_name_edited_on_two_devices_keeps_the_first_and_the_other_is_reapplied() {
        with_each_database(|database| async move {
            let (server, _storage) = server(database);
            let (alice, token) = user_with_token(&server, "alice@example.com").await;
            let (a, _a_folder) = device_with_folder(&alice, &token).await;
            let (b, _b_folder) = device_with_folder(&alice, &token).await;
            let transport = InProcess { server: &server };
            let file = with_bytes(&a, &alice, "notes.txt", "text/plain", b"some notes")
                .await
                .expect("a file");
            sync::run(&a, &transport).await.expect("a sync");
            sync::run(&b, &transport).await.expect("a sync");

            rename(&a, &alice, file.id, "from a.txt")
                .await
                .expect("renamed on a");
            rename(&b, &alice, file.id, "from b.txt")
                .await
                .expect("renamed on b");
            sync::run(&a, &transport).await.expect("a sync");
            let report = sync::run(&b, &transport).await.expect("a sync");
            assert_eq!(report.conflicts, 1);
            assert_eq!(
                get(&b, &alice, file.id).await.expect("on b").file.name,
                "from a.txt"
            );
            let changes = pending_changes::list(&b).await.expect("none waiting");
            assert!(changes.is_empty());
            let listed = conflicts::list(&b).await.expect("the conflicts");
            assert_eq!(listed[0].about.as_deref(), Some("from a.txt"));
            assert_eq!(listed[0].resolve, Resolve::Reapply);
            assert_eq!(listed[0].fields[0].yours, json!("from b.txt"));

            conflicts::reapply(&b, &alice, listed[0].id)
                .await
                .expect("reapplied");
            assert_eq!(
                pending_changes::list(&b).await.expect("one waiting")[0]
                    .about
                    .as_deref(),
                Some("from b.txt")
            );
            sync::run(&b, &transport).await.expect("a sync");
            assert_eq!(
                get(&server, &alice, file.id)
                    .await
                    .expect("on the server")
                    .file
                    .name,
                "from b.txt"
            );
        })
        .await;
    }

    #[tokio::test]
    async fn files_delete_unreferenced_sweeps_the_bucket_of_bytes_no_row_points_at() {
        with_each_database(|database| async move {
            let (server, storage) = server(database);
            let alice = users::create(&server, "alice@example.com", "Alice")
                .await
                .expect("a user");
            let kept = uploaded(&server, &storage, &alice, "kept.txt", b"kept").await;
            let deleted = uploaded(&server, &storage, &alice, "deleted.txt", b"deleted").await;
            delete(&server, &alice, deleted.id).await.expect("deleted");
            let never_pushed = Uuid::now_v7();
            storage
                .put(&storage_key(alice.id, never_pushed), b"orphan".to_vec())
                .await
                .expect("an upload whose row never came");
            let now = now();

            let report = delete_unreferenced(&server, now).await.expect("a sweep");
            assert_eq!(
                (report.kept, report.deleted),
                (3, 0),
                "nothing is a day old"
            );

            let tomorrow = now + chrono::Duration::days(UNREFERENCED_DAYS + 1);
            let report = delete_unreferenced(&server, tomorrow)
                .await
                .expect("a sweep");
            assert_eq!((report.kept, report.deleted), (2, 1));
            assert_eq!(in_bucket(&storage, &alice, never_pushed).await, None);
            assert_eq!(
                in_bucket(&storage, &alice, deleted.id).await,
                Some(7),
                "a deleted row keeps its bytes until the purge"
            );

            let after_the_window =
                now + chrono::Duration::days(crate::app::purge::RETENTION_DAYS + 1);
            crate::app::purge::run(&server, after_the_window)
                .await
                .expect("purged");
            let report = delete_unreferenced(&server, after_the_window)
                .await
                .expect("a sweep");
            assert_eq!((report.kept, report.deleted), (1, 1));
            assert_eq!(in_bucket(&storage, &alice, deleted.id).await, None);
            assert_eq!(in_bucket(&storage, &alice, kept.id).await, Some(4));
        })
        .await;
    }

    #[tokio::test]
    async fn files_a_deleted_files_bytes_leave_every_device_at_its_next_sync() {
        with_each_database(|database| async move {
            let (server, _storage) = server(database);
            let (alice, token) = user_with_token(&server, "alice@example.com").await;
            let (a, a_folder) = device_with_folder(&alice, &token).await;
            let (b, b_folder) = device_with_folder(&alice, &token).await;
            let transport = InProcess { server: &server };
            let file = with_bytes(&a, &alice, "notes.txt", "text/plain", b"some notes")
                .await
                .expect("a file");
            sync::run(&a, &transport).await.expect("a sync");
            sync::run(&b, &transport).await.expect("a sync");
            local_copy(&b, &transport, &alice, file.id)
                .await
                .expect("fetched on b");

            delete(&a, &alice, file.id).await.expect("deleted on a");
            let report = sync::run(&a, &transport).await.expect("a sync");
            assert_eq!(report.cleaned, 1);
            assert_eq!(
                std::fs::read_dir(a_folder.path())
                    .expect("the folder")
                    .count(),
                0
            );
            let report = sync::run(&b, &transport).await.expect("a sync");
            assert_eq!(report.cleaned, 1);
            assert_eq!(
                std::fs::read_dir(b_folder.path())
                    .expect("the folder")
                    .count(),
                0
            );
        })
        .await;
    }

    #[tokio::test]
    async fn files_delete_unreferenced_on_device_leaves_new_bytes_and_every_file_not_yet_synced() {
        let alice = users::tests::alice();
        let (device, folder) = device_with_folder(&alice, "a token").await;
        let waiting = with_bytes(&device, &alice, "notes.txt", "text/plain", b"x")
            .await
            .expect("a file");
        let without_a_row = folder.path().join(Uuid::now_v7().hyphenated().to_string());
        std::fs::write(&without_a_row, b"written a moment before its row").expect("bytes");
        let half_fetched = folder.path().join("0192f7c0.part");
        std::fs::write(&half_fetched, b"half").expect("bytes");

        let removed = delete_unreferenced_on_device(&device, now())
            .await
            .expect("cleaned");
        assert_eq!(removed, 0, "nothing without a row is a day old");

        let tomorrow = now() + chrono::Duration::days(UNREFERENCED_DAYS + 1);
        let removed = delete_unreferenced_on_device(&device, tomorrow)
            .await
            .expect("cleaned");
        assert_eq!(removed, 2);
        assert!(!without_a_row.exists());
        assert!(!half_fetched.exists());
        assert!(
            local_path(&device, waiting.id).expect("a path").exists(),
            "a waiting file is the only copy, however old"
        );
    }

    #[tokio::test]
    async fn files_create_needs_the_bytes_in_the_bucket() {
        with_each_database(|database| async move {
            let (deployment, storage) = server(database);
            let alice = users::create(&deployment, "alice@example.com", "Alice")
                .await
                .expect("a user");
            let new = new_file(
                "notes.txt",
                b"some notes",
                personal(&deployment, &alice).await,
            );

            let error = create(&deployment, &alice, &new)
                .await
                .expect_err("no bytes yet");
            assert!(matches!(error, FileError::NotUploaded));
            assert!(
                list(&deployment, &alice)
                    .await
                    .expect("the files")
                    .is_empty()
            );

            storage
                .put(&storage_key(alice.id, new.id), b"other".to_vec())
                .await
                .expect("an upload of another size");
            let error = create(&deployment, &alice, &new)
                .await
                .expect_err("not the size it said");
            assert!(matches!(error, FileError::NotUploaded));

            storage
                .put(&storage_key(alice.id, new.id), b"some other".to_vec())
                .await
                .expect("an upload of the size, other bytes");
            let error = create(&deployment, &alice, &new)
                .await
                .expect_err("not the checksum it said");
            assert!(matches!(error, FileError::NotUploaded));

            storage
                .put(&storage_key(alice.id, new.id), b"some notes".to_vec())
                .await
                .expect("the upload");
            let file = create(&deployment, &alice, &new).await.expect("a file");
            assert_eq!(file.id, new.id);
            assert_eq!(file.created_by_id, alice.id);
            assert_eq!(file.size, 10);
            assert_eq!(file.version, 1);
            let listed = list(&deployment, &alice).await.expect("the files");
            assert_eq!(listed.len(), 1);
            assert_eq!(listed[0].file, file);
            assert_eq!(listed[0].state, FileState::Synced);
            assert_eq!(listed[0].shown_as, ShownAs::Text);
            assert_eq!(ShownAs::of("image/gif"), ShownAs::Image);
            assert_eq!(ShownAs::of("application/pdf"), ShownAs::Pdf);
            assert_eq!(ShownAs::of("text/html"), ShownAs::NotShown);
        })
        .await;
    }

    #[tokio::test]
    async fn files_create_trims_the_name_and_refuses_what_is_outside_the_rules() {
        with_each_database(|database| async move {
            let (deployment, storage) = server(database);
            let alice = users::create(&deployment, "alice@example.com", "Alice")
                .await
                .expect("a user");
            let file = uploaded(&deployment, &storage, &alice, "  spaced.txt ", b"x").await;
            assert_eq!(file.name, "spaced.txt");

            let wrong = FileNew {
                id: Uuid::now_v7(),
                name: "  ".to_string(),
                content_type: "not a type".to_string(),
                size: MAX_BYTES + 1,
                sha256: "not a checksum".to_string(),
                community_ids: Vec::new(),
            };
            let error = create(&deployment, &alice, &wrong)
                .await
                .expect_err("three fields wrong");
            let FileError::Invalid(errors) = error else {
                panic!("field errors, got {error}");
            };
            assert_eq!(errors.0["name"], ["is blank"]);
            assert_eq!(errors.0["content_type"], ["is not a content type"]);
            assert_eq!(errors.0["size"], ["is larger than 25 MB"]);
            assert_eq!(errors.0["sha256"], ["is not a SHA-256"]);

            let untyped = FileNew {
                content_type: String::new(),
                ..new_file("data", b"x", personal(&deployment, &alice).await)
            };
            storage
                .put(&storage_key(alice.id, untyped.id), b"x".to_vec())
                .await
                .expect("the upload");
            let file = create(&deployment, &alice, &untyped)
                .await
                .expect("a file of no known type");
            assert_eq!(file.content_type, UNKNOWN_CONTENT_TYPE);
        })
        .await;
    }

    #[tokio::test]
    async fn files_search_finds_names_in_any_case_a_page_at_a_time() {
        with_each_database(|database| async move {
            let (deployment, storage) = server(database);
            let alice = users::create(&deployment, "alice@example.com", "Alice")
                .await
                .expect("a user");
            let bob = users::create(&deployment, "bob@example.com", "Bob")
                .await
                .expect("a user");
            for (name, bytes) in [
                ("Cat.gif", b"1"),
                ("dog.gif", b"2"),
                ("caterpillar.png", b"3"),
                ("100%_done.txt", b"4"),
            ] {
                uploaded(&deployment, &storage, &alice, name, bytes).await;
            }
            let found = |name: &str, offset: u32, limit: u32| {
                let deployment = deployment.clone();
                let alice = alice.clone();
                let asked = FileSearch {
                    name: name.to_string(),
                    offset,
                    limit,
                    ..FileSearch::default()
                };
                async move {
                    let results = search(&deployment, &alice, &asked).await.expect("a search");
                    let names: Vec<String> = results
                        .files
                        .into_iter()
                        .map(|listed| listed.file.name)
                        .collect();
                    (results.total, names)
                }
            };

            assert_eq!(
                found("", 0, 10).await,
                (
                    4,
                    vec![
                        "100%_done.txt".to_string(),
                        "caterpillar.png".to_string(),
                        "dog.gif".to_string(),
                        "Cat.gif".to_string(),
                    ]
                )
            );
            assert_eq!(
                found("CAT", 0, 10).await,
                (
                    2,
                    vec!["caterpillar.png".to_string(), "Cat.gif".to_string()]
                )
            );
            assert_eq!(found("cat", 1, 1).await, (2, vec!["Cat.gif".to_string()]));
            assert_eq!(found("cat", 2, 10).await, (2, Vec::new()));
            assert_eq!(
                found("%", 0, 10).await,
                (1, vec!["100%_done.txt".to_string()])
            );
            assert_eq!(
                found("_", 0, 10).await,
                (1, vec!["100%_done.txt".to_string()])
            );

            let theirs = FileSearch {
                limit: 10,
                ..FileSearch::default()
            };
            let results = search(&deployment, &bob, &theirs).await.expect("a search");
            assert_eq!((results.total, results.files.len()), (0, 0));

            let too_many = FileSearch {
                limit: MAX_SEARCH_LIMIT + 1,
                ..FileSearch::default()
            };
            let error = search(&deployment, &alice, &too_many)
                .await
                .expect_err("over the limit");
            let FileError::Invalid(errors) = error else {
                panic!("field errors, got {error}");
            };
            assert_eq!(errors.0["limit"], ["at most 500 files at once"]);
        })
        .await;
    }

    #[tokio::test]
    async fn files_search_keeps_to_the_content_types_asked_for() {
        with_each_database(|database| async move {
            let (deployment, storage) = server(database);
            let alice = users::create(&deployment, "alice@example.com", "Alice")
                .await
                .expect("a user");
            let community_id = personal(&deployment, &alice).await;
            for (name, content_type, bytes) in [
                ("cat.gif", "image/gif", b"1"),
                ("cat.png", "image/png; charset=binary", b"2"),
                ("cat.svg", "image/svg+xml", b"3"),
                ("cat.txt", "text/plain", b"4"),
            ] {
                let new = FileNew {
                    content_type: content_type.to_string(),
                    ..new_file(name, bytes, community_id)
                };
                storage
                    .put(&storage_key(alice.id, new.id), bytes.to_vec())
                    .await
                    .expect("the upload");
                create(&deployment, &alice, &new).await.expect("a file");
            }
            let found = |content_types: &[&str]| {
                let deployment = deployment.clone();
                let alice = alice.clone();
                let asked = FileSearch {
                    name: "CAT".to_string(),
                    content_types: content_types.iter().map(ToString::to_string).collect(),
                    offset: 0,
                    limit: 10,
                };
                async move {
                    let results = search(&deployment, &alice, &asked).await.expect("a search");
                    let names: Vec<String> = results
                        .files
                        .into_iter()
                        .map(|listed| listed.file.name)
                        .collect();
                    (results.total, names)
                }
            };

            assert_eq!(
                found(&["image/gif", "image/png"]).await,
                (2, vec!["cat.png".to_string(), "cat.gif".to_string()])
            );
            assert_eq!(found(&[]).await.0, 4);
            assert_eq!(found(&["image/jpeg"]).await, (0, Vec::new()));
        })
        .await;
    }

    #[tokio::test]
    async fn files_list_and_get_show_a_user_no_file_outside_their_communities() {
        with_each_database(|database| async move {
            let (deployment, storage) = server(database);
            let alice = users::create(&deployment, "alice@example.com", "Alice")
                .await
                .expect("a user");
            let bob = users::create(&deployment, "bob@example.com", "Bob")
                .await
                .expect("a user");
            let first = uploaded(&deployment, &storage, &alice, "first.txt", b"1").await;
            let second = uploaded(&deployment, &storage, &alice, "second.txt", b"2").await;

            let names: Vec<String> = list(&deployment, &alice)
                .await
                .expect("the files")
                .into_iter()
                .map(|listed| listed.file.name)
                .collect();
            assert_eq!(names, ["second.txt", "first.txt"]);
            assert_eq!(
                get(&deployment, &alice, first.id).await.expect("hers").file,
                first
            );
            assert!(list(&deployment, &bob).await.expect("the files").is_empty());
            for error in [
                get(&deployment, &bob, second.id)
                    .await
                    .expect_err("not his"),
                rename(&deployment, &bob, second.id, "his.txt")
                    .await
                    .expect_err("not his"),
                delete(&deployment, &bob, second.id)
                    .await
                    .expect_err("not his"),
                content_url(&deployment, &bob, second.id, false)
                    .await
                    .expect_err("not his"),
            ] {
                assert!(matches!(error, FileError::NotFound { .. }), "{error}");
            }
        })
        .await;
    }

    #[tokio::test]
    async fn files_update_changes_the_name_only() {
        with_each_database(|database| async move {
            let (deployment, storage) = server(database);
            let alice = users::create(&deployment, "alice@example.com", "Alice")
                .await
                .expect("a user");
            let file = uploaded(&deployment, &storage, &alice, "before.txt", b"x").await;

            let updated = rename(&deployment, &alice, file.id, " after.txt ")
                .await
                .expect("renamed");
            assert_eq!(updated.name, "after.txt");
            assert_eq!(updated.version, 2);
            assert_eq!(updated.size, file.size);
            let same = rename(&deployment, &alice, file.id, "after.txt")
                .await
                .expect("unchanged");
            assert_eq!(same.version, 2);
            let error = rename(&deployment, &alice, file.id, " ")
                .await
                .expect_err("blank");
            assert!(matches!(error, FileError::Invalid(_)));

            let event = Event::update(
                TABLE,
                file.id,
                2,
                json::object_fields(json!({ "size": 1 })),
                json::object_fields(json!({ "size": 2 })),
                now(),
            );
            let answer = apply_event(&deployment, &alice, &event)
                .await
                .expect("answered");
            assert_eq!(answer.status, AnswerStatus::Rejected);
            assert_eq!(answer.reason.as_deref(), Some("not_editable: size"));
        })
        .await;
    }

    #[tokio::test]
    async fn files_delete_hides_the_file_and_records_who() {
        with_each_database(|database| async move {
            let (deployment, storage) = server(database);
            let alice = users::create(&deployment, "alice@example.com", "Alice")
                .await
                .expect("a user");
            let file = uploaded(&deployment, &storage, &alice, "going.txt", b"x").await;

            delete(&deployment, &alice, file.id).await.expect("deleted");
            let error = get(&deployment, &alice, file.id).await.expect_err("gone");
            assert!(matches!(error, FileError::NotFound { .. }));
            assert!(
                list(&deployment, &alice)
                    .await
                    .expect("the files")
                    .is_empty()
            );
            let mut tx = deployment.begin().await.expect("a transaction");
            let row = get_in(&mut tx, file.id)
                .await
                .expect("the row")
                .expect("kept");
            assert!(row.deleted_at.is_some());
            assert_eq!(row.deleted_by_id, Some(alice.id));
            assert_eq!(
                storage
                    .size(&storage_key(alice.id, file.id))
                    .await
                    .expect("looked"),
                Some(1),
                "the bytes stay for the sweep"
            );
        })
        .await;
    }

    #[tokio::test]
    async fn files_create_upload_signs_a_url_until_the_bytes_are_there() {
        with_each_database(|database| async move {
            let (deployment, storage) = server(database);
            let alice = users::create(&deployment, "alice@example.com", "Alice")
                .await
                .expect("a user");
            let request = FileUploadNew {
                id: None,
                content_type: "IMAGE/GIF".to_string(),
                size: 3,
                sha256: sha256::hex(b"GIF"),
            };

            let upload = create_upload(&deployment, &alice, &request)
                .await
                .expect("where to upload");
            assert!(!upload.uploaded);
            assert_eq!(upload.content_type, "image/gif");
            assert_eq!(
                upload.checksum_sha256,
                "dsZk7xUuBlki/tRycxXQZbj7Gu1hAVza73vP6jxY1as="
            );
            let key = storage_key(alice.id, upload.id);
            assert_eq!(upload.upload_url, Some(format!("memory:///{key}")));

            storage
                .put(&key, b"GIF".to_vec())
                .await
                .expect("the upload");
            let again = FileUploadNew {
                id: Some(upload.id),
                ..request.clone()
            };
            let upload = create_upload(&deployment, &alice, &again)
                .await
                .expect("already there");
            assert!(upload.uploaded);
            assert_eq!(upload.upload_url, None);

            let too_large = FileUploadNew {
                size: MAX_BYTES + 1,
                ..request
            };
            let error = create_upload(&deployment, &alice, &too_large)
                .await
                .expect_err("too large");
            assert!(matches!(error, FileError::Invalid(_)));
        })
        .await;
    }

    #[tokio::test]
    async fn files_content_url_is_signed_under_the_creator() {
        with_each_database(|database| async move {
            let (deployment, storage) = server(database);
            let alice = users::create(&deployment, "alice@example.com", "Alice")
                .await
                .expect("a user");
            let file = uploaded(&deployment, &storage, &alice, "notes.txt", b"x").await;
            let url = content_url(&deployment, &alice, file.id, false)
                .await
                .expect("a url");
            assert_eq!(url, format!("memory:///{}", storage_key(alice.id, file.id)));
            assert_eq!(
                attachment_named("my notes \"v2\".txt"),
                "attachment; filename*=UTF-8''my%20notes%20%22v2%22.txt"
            );
        })
        .await;
    }

    #[tokio::test]
    async fn files_without_a_bucket_the_server_refuses_to_create() {
        let database = Database::open_sqlite_in_memory().await.expect("a database");
        let deployment = Deployment::new(database, Role::Server);
        let alice = users::create(&deployment, "alice@example.com", "Alice")
            .await
            .expect("a user");
        let community_id = personal(&deployment, &alice).await;
        let error = create(
            &deployment,
            &alice,
            &new_file("notes.txt", b"x", community_id),
        )
        .await
        .expect_err("no bucket");
        assert!(matches!(error, FileError::NoFileStore));
    }
}
