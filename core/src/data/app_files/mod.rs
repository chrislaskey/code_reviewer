//! File records: the `File` schema, its baseline validation, and the
//! reads and writes of `app_files`. Callers own and commit the
//! transaction. Writes are crate-private: a file syncs, so each write is
//! an event applied by `app::files`.

use chrono::{DateTime, Utc};
use garde::Validate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::data::app_sync_state::Cursor;
use crate::database::Transaction;
use crate::helpers::field_errors::FieldErrors;

pub const TABLE: &str = "app_files";
/// The largest file, 25 MB.
pub const MAX_BYTES: u64 = 25 * 1024 * 1024;
/// The longest name, in characters.
pub const MAX_NAME_LENGTH: usize = 255;
/// What a file with no content type of its own is sent as.
pub const UNKNOWN_CONTENT_TYPE: &str = "application/octet-stream";

/// A file's row, with the sync columns every synced row carries, see
/// `docs/design/offline-sync.md`. The rules on its fields are what
/// `AcceptedFile::new` checks.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Validate)]
pub struct File {
    #[garde(skip)]
    pub id: Uuid,
    /// Who created it: a record, not an owner, see `files_communities`.
    #[garde(skip)]
    pub created_by_id: Uuid,
    #[garde(custom(is_file_name))]
    pub name: String,
    #[garde(custom(is_content_type))]
    pub content_type: String,
    /// In bytes.
    #[garde(custom(is_file_size))]
    pub size: u64,
    /// The bytes' SHA-256, lowercase hex.
    #[garde(custom(is_sha256))]
    pub sha256: String,
    #[garde(skip)]
    pub inserted_at: DateTime<Utc>,
    #[garde(skip)]
    pub updated_at: DateTime<Utc>,
    /// The server's version, 0 on a device's file not yet pushed.
    #[garde(skip)]
    pub version: i64,
    /// When the server last wrote it; none on a device's file not yet pushed.
    #[garde(skip)]
    pub last_synced_at: Option<DateTime<Utc>>,
    #[garde(skip)]
    pub deleted_at: Option<DateTime<Utc>>,
    #[garde(skip)]
    pub deleted_by_id: Option<Uuid>,
}

/// A file that passed validation, which `create` and `update` require.
pub struct AcceptedFile(File);

impl AcceptedFile {
    /// Checks the file, with its name trimmed and a missing content type
    /// filled in.
    pub fn new(file: File) -> Result<AcceptedFile, FieldErrors> {
        let file = File {
            name: file.name.trim().to_string(),
            content_type: content_type_or_unknown(&file.content_type),
            ..file
        };
        FieldErrors::check(&file)?;
        Ok(AcceptedFile(file))
    }

    /// The accepted file.
    pub fn row(&self) -> &File {
        &self.0
    }

    /// The accepted file, owned.
    pub fn into_row(self) -> File {
        self.0
    }
}

/// The content type a browser gave, or the unknown one when it gave none.
pub fn content_type_or_unknown(content_type: &str) -> String {
    match content_type.trim() {
        "" => UNKNOWN_CONTENT_TYPE.to_string(),
        content_type => content_type.to_lowercase(),
    }
}

/// A name that is not blank and not too long.
pub fn is_file_name(value: &str, _context: &()) -> garde::Result {
    if value.trim().is_empty() {
        return Err(garde::Error::new("is blank"));
    }
    if value.chars().count() > MAX_NAME_LENGTH {
        return Err(garde::Error::new("is longer than 255 characters"));
    }
    Ok(())
}

/// A `type/subtype`, with parameters or not, in printable ASCII.
pub fn is_content_type(value: &str, _context: &()) -> garde::Result {
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
pub fn is_sha256(value: &str, _context: &()) -> garde::Result {
    let hex = value
        .chars()
        .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c));
    if value.len() != 64 || !hex {
        return Err(garde::Error::new("is not a SHA-256"));
    }
    Ok(())
}

/// A size that is not empty and not too large.
pub fn is_file_size(value: &u64, _context: &()) -> garde::Result {
    if *value == 0 {
        return Err(garde::Error::new("is empty"));
    }
    if *value > MAX_BYTES {
        return Err(garde::Error::new("is larger than 25 MB"));
    }
    Ok(())
}

#[cfg(feature = "postgres")]
mod postgres;
mod sqlite;

macro_rules! query {
    ($tx:expr, $function:ident($($argument:expr),* $(,)?)) => {
        match $tx {
            Transaction::Sqlite(tx) => sqlite::$function(&mut **tx, $($argument),*).await,
            #[cfg(feature = "postgres")]
            Transaction::Postgres(tx) => postgres::$function(&mut **tx, $($argument),*).await,
        }
    };
}

/// Lists the live files the user sees, newest first with an ID tie-breaker.
pub async fn list(tx: &mut Transaction, user_id: Uuid) -> Result<Vec<File>, sqlx::Error> {
    query!(tx, list_visible(user_id))
}

/// Counts the live files the user sees whose names contain `name` in
/// any case, of one of `content_types` (any for none), and lists `limit`
/// of them from `offset`, newest first.
pub(crate) async fn search(
    tx: &mut Transaction,
    user_id: Uuid,
    name: &str,
    content_types: &[String],
    offset: i64,
    limit: i64,
) -> Result<(u64, Vec<File>), sqlx::Error> {
    query!(
        tx,
        search_visible(user_id, name, content_types, offset, limit)
    )
}

/// Lists the live files the user sees with any of these checksums,
/// newest first.
pub async fn list_with_sha256s(
    tx: &mut Transaction,
    user_id: Uuid,
    sha256s: &[String],
) -> Result<Vec<File>, sqlx::Error> {
    query!(tx, list_visible_with_sha256s(user_id, sha256s))
}

/// Gets a live file the user sees.
pub async fn get(
    tx: &mut Transaction,
    user_id: Uuid,
    id: Uuid,
) -> Result<Option<File>, sqlx::Error> {
    query!(tx, get_visible(user_id, id))
}

/// Persists a row accepted by the resource workflow.
pub(crate) async fn create(tx: &mut Transaction, file: &AcceptedFile) -> Result<(), sqlx::Error> {
    query!(tx, insert(file.row()))
}

/// Persists a row accepted by the resource workflow.
pub(crate) async fn update(tx: &mut Transaction, file: &AcceptedFile) -> Result<bool, sqlx::Error> {
    query!(tx, update(file.row()))
}

/// Explicit replication access for internal resource workflows.
pub mod replication {
    use super::*;

    /// Reads protocol state, including tombstones, in the caller transaction.
    pub async fn get(tx: &mut Transaction, id: Uuid) -> Result<Option<File>, sqlx::Error> {
        query!(tx, get(id))
    }

    /// Reads protocol state, including tombstones, in the caller transaction.
    pub async fn list_synced_after(
        tx: &mut Transaction,
        user_id: Uuid,
        after: &Cursor,
        limit: usize,
    ) -> Result<Vec<File>, sqlx::Error> {
        query!(tx, list_synced_after(user_id, after, limit))
    }

    /// Persists a row accepted by the resource workflow.
    pub(crate) async fn upsert(tx: &mut Transaction, file: &File) -> Result<(), sqlx::Error> {
        query!(tx, upsert(file))
    }
}

/// Explicit maintenance access for internal resource workflows.
pub(crate) mod maintenance {
    use super::*;

    /// Lists row IDs, creators and deletion state for byte cleanup.
    pub async fn list_ids(tx: &mut Transaction) -> Result<Vec<(Uuid, Uuid, bool)>, sqlx::Error> {
        query!(tx, list_ids())
    }

    /// Lists the files on this database the user does not see, for a prune.
    pub async fn list_invisible_ids(
        tx: &mut Transaction,
        user_id: Uuid,
    ) -> Result<Vec<Uuid>, sqlx::Error> {
        query!(tx, list_invisible_ids(user_id))
    }

    /// Physically removes a refused, forgotten or pruned local file.
    pub(crate) async fn delete(tx: &mut Transaction, id: Uuid) -> Result<(), sqlx::Error> {
        query!(tx, delete_row(id))
    }
}
