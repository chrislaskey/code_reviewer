//! File records: the `File` schema, its baseline validation, and the
//! reads and writes of `app_files`. Callers own and commit the
//! transaction. Writes are crate-private: a file syncs, so each write is
//! an event applied by `app::files`.

use chrono::{DateTime, Utc};
use garde::Validate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::data::app_sync_changes;
use crate::database::Transaction;
use crate::deployment::Role;
use crate::helpers::content_type::{content_type_or_unknown, is_content_type};
use crate::helpers::field_errors::FieldErrors;
use crate::helpers::sha256::{self, is_sha256};
use crate::sync::VersionRange;

pub const TABLE: &str = "app_files";
/// The largest file, 25 MB.
pub const MAX_BYTES: u64 = 25 * 1024 * 1024;
/// The most content a row keeps itself, 64 KB: every member's device
/// gets it with the row.
pub const MAX_CONTENT_BYTES: usize = 64 * 1024;
/// The longest name, in characters.
pub const MAX_NAME_LENGTH: usize = 255;

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
    /// The content, when the row keeps it; none when the bytes are in
    /// the bucket. See `docs/design/files-content.md`.
    #[garde(custom(is_file_content))]
    #[serde(default)]
    pub content: Option<String>,
    #[garde(skip)]
    pub inserted_at: DateTime<Utc>,
    #[garde(skip)]
    pub updated_at: DateTime<Utc>,
    /// The server's version, 0 on a device's file not yet pushed.
    #[garde(skip)]
    pub version: i64,
    #[garde(skip)]
    #[serde(with = "crate::sync::version::optional")]
    pub global_sync_version: Option<i64>,
    #[garde(skip)]
    pub deleted_at: Option<DateTime<Utc>>,
    #[garde(skip)]
    pub deleted_by_id: Option<Uuid>,
}

/// A file that passed validation, which `create` and `update` require.
pub struct AcceptedFile(File);

impl AcceptedFile {
    /// Checks the file, with its name trimmed, a missing content type
    /// filled in, and the size and SHA-256 of content the row keeps.
    pub fn new(file: File) -> Result<AcceptedFile, FieldErrors> {
        let (size, sha256) = match &file.content {
            Some(content) => (content.len() as u64, sha256::hex(content.as_bytes())),
            None => (file.size, file.sha256.clone()),
        };
        let file = File {
            name: file.name.trim().to_string(),
            content_type: content_type_or_unknown(&file.content_type),
            size,
            sha256,
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

/// Content a row keeps: not empty and not too large.
pub fn is_file_content(value: &Option<String>, _context: &()) -> garde::Result {
    let Some(content) = value else {
        return Ok(());
    };
    if content.is_empty() {
        return Err(garde::Error::new("is empty"));
    }
    if content.len() > MAX_CONTENT_BYTES {
        return Err(garde::Error::new("is larger than 64 KB"));
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

/// A required read scope.
pub enum Filter<'a> {
    /// The live files the user sees, newest first with an ID tie-breaker.
    VisibleTo(Uuid),
    /// The live files the user sees with any of these checksums, newest
    /// first.
    VisibleToWithSha256s(Uuid, &'a [String]),
}

/// Lists live rows within the required scope.
pub async fn list(tx: &mut Transaction, filter: Filter<'_>) -> Result<Vec<File>, sqlx::Error> {
    match filter {
        Filter::VisibleTo(user_id) => query!(tx, list_visible(user_id)),
        Filter::VisibleToWithSha256s(user_id, sha256s) => {
            query!(tx, list_visible_with_sha256s(user_id, sha256s))
        }
    }
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

/// Gets a live file the user sees.
pub async fn get(
    tx: &mut Transaction,
    user_id: Uuid,
    id: Uuid,
) -> Result<Option<File>, sqlx::Error> {
    query!(tx, get_visible(user_id, id))
}

/// Inserts a new row. On the server this publishes it: the row gets the
/// next global sync version, recorded in `app_sync_changes` in this
/// transaction. On a device the row keeps the version it has.
pub(crate) async fn create(
    tx: &mut Transaction,
    role: Role,
    file: AcceptedFile,
) -> Result<File, sqlx::Error> {
    let mut file = file.into_row();
    file.global_sync_version =
        app_sync_changes::stamp(tx, role, TABLE, file.id, file.global_sync_version).await?;
    query!(tx, insert(&file))?;
    Ok(file)
}

/// Writes the row over its stored one, a soft delete included, and
/// publishes it on the server as `create` does.
pub(crate) async fn update(
    tx: &mut Transaction,
    role: Role,
    file: AcceptedFile,
) -> Result<File, sqlx::Error> {
    let mut file = file.into_row();
    file.global_sync_version =
        app_sync_changes::stamp(tx, role, TABLE, file.id, file.global_sync_version).await?;
    query!(tx, update(&file))?;
    Ok(file)
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
        range: &VersionRange,
        limit: usize,
    ) -> Result<Vec<File>, sqlx::Error> {
        query!(tx, list_synced_after(user_id, range, limit))
    }

    /// Persists a row accepted by the resource workflow.
    pub(crate) async fn upsert(tx: &mut Transaction, file: &File) -> Result<(), sqlx::Error> {
        query!(tx, upsert(file))
    }
}

/// Explicit maintenance access for internal resource workflows.
pub(crate) mod maintenance {
    use super::*;

    /// Drops the files for a resync, but the pending and the refused.
    pub(crate) async fn delete_for_resync(tx: &mut Transaction) -> Result<(), sqlx::Error> {
        query!(tx, delete_for_resync())
    }

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
