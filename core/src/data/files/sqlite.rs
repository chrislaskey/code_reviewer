//! The `app_files` table. Ids are UUIDv7 text, so the file reads well in
//! the sqlite3 shell; the row type here turns them into `File`'s `Uuid`.
//! A user sees a file in a community they are a live member of.

use chrono::{DateTime, Utc};
use sqlx::{FromRow, SqliteConnection};
use uuid::Uuid;
use uuid::fmt::Hyphenated;

use crate::files::File;
use crate::sync::Cursor;

#[derive(FromRow)]
struct FileRow {
    id: Hyphenated,
    created_by_id: Hyphenated,
    name: String,
    content_type: String,
    size: i64,
    sha256: String,
    inserted_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    version: i64,
    last_synced_at: Option<DateTime<Utc>>,
    deleted_at: Option<DateTime<Utc>>,
    deleted_by_id: Option<Hyphenated>,
}

impl From<FileRow> for File {
    fn from(row: FileRow) -> File {
        File {
            id: row.id.into_uuid(),
            created_by_id: row.created_by_id.into_uuid(),
            name: row.name,
            content_type: row.content_type,
            size: u64::try_from(row.size).unwrap_or_default(),
            sha256: row.sha256,
            inserted_at: row.inserted_at,
            updated_at: row.updated_at,
            version: row.version,
            last_synced_at: row.last_synced_at,
            deleted_at: row.deleted_at,
            deleted_by_id: row.deleted_by_id.map(Hyphenated::into_uuid),
        }
    }
}

macro_rules! select {
    ($($rest:expr),+ $(,)?) => {
        concat!(
            "SELECT id, created_by_id, name, content_type, size, sha256, inserted_at, updated_at, version, last_synced_at, deleted_at, deleted_by_id FROM app_files ",
            $($rest),+
        )
    };
}

/// The files the user, `$1`, sees: those with a live row in a community
/// they are a live member of.
macro_rules! visible_to {
    () => {
        "id IN (SELECT fc.file_id FROM app_files_communities fc
            WHERE fc.deleted_at IS NULL AND fc.community_id IN (SELECT community_id FROM app_users_communities WHERE user_id = $1 AND deleted_at IS NULL))"
    };
}

fn size_of(file: &File) -> i64 {
    i64::try_from(file.size).unwrap_or(i64::MAX)
}

/// The files the user sees, not deleted, newest first.
pub async fn list_visible(
    connection: &mut SqliteConnection,
    user_id: Uuid,
) -> Result<Vec<File>, sqlx::Error> {
    let rows: Vec<FileRow> = sqlx::query_as(select!(
        "WHERE deleted_at IS NULL AND ",
        visible_to!(),
        " ORDER BY inserted_at DESC, id DESC"
    ))
    .bind(user_id.hyphenated())
    .fetch_all(connection)
    .await?;
    Ok(rows.into_iter().map(File::from).collect())
}

/// The files the user sees with these bytes, not deleted, newest first.
pub async fn list_visible_with_sha256(
    connection: &mut SqliteConnection,
    user_id: Uuid,
    sha256: &str,
) -> Result<Vec<File>, sqlx::Error> {
    let rows: Vec<FileRow> = sqlx::query_as(select!(
        "WHERE deleted_at IS NULL AND sha256 = $2 AND ",
        visible_to!(),
        " ORDER BY inserted_at DESC, id DESC"
    ))
    .bind(user_id.hyphenated())
    .bind(sha256)
    .fetch_all(connection)
    .await?;
    Ok(rows.into_iter().map(File::from).collect())
}

/// The file by its id if the user sees it, not deleted.
pub async fn get_visible(
    connection: &mut SqliteConnection,
    user_id: Uuid,
    id: Uuid,
) -> Result<Option<File>, sqlx::Error> {
    let row: Option<FileRow> = sqlx::query_as(select!(
        "WHERE id = $2 AND deleted_at IS NULL AND ",
        visible_to!()
    ))
    .bind(user_id.hyphenated())
    .bind(id.hyphenated())
    .fetch_optional(connection)
    .await?;
    Ok(row.map(File::from))
}

/// The ids of the files on this database the user does not see: what a
/// device prunes.
pub async fn list_invisible_ids(
    connection: &mut SqliteConnection,
    user_id: Uuid,
) -> Result<Vec<Uuid>, sqlx::Error> {
    let rows: Vec<(Hyphenated,)> = sqlx::query_as(concat!(
        "SELECT id FROM app_files WHERE NOT ",
        visible_to!()
    ))
    .bind(user_id.hyphenated())
    .fetch_all(connection)
    .await?;
    Ok(rows.into_iter().map(|(id,)| id.into_uuid()).collect())
}

/// The row by its id, deleted or not, whoever sees it.
pub async fn get(connection: &mut SqliteConnection, id: Uuid) -> Result<Option<File>, sqlx::Error> {
    let row: Option<FileRow> = sqlx::query_as(select!("WHERE id = $1"))
        .bind(id.hyphenated())
        .fetch_optional(connection)
        .await?;
    Ok(row.map(File::from))
}

/// Every row's id and creator, with whether it is deleted: what the
/// bytes kept for the rows are checked against.
pub async fn list_ids(
    connection: &mut SqliteConnection,
) -> Result<Vec<(Uuid, Uuid, bool)>, sqlx::Error> {
    let rows: Vec<(Hyphenated, Hyphenated, bool)> =
        sqlx::query_as("SELECT id, created_by_id, deleted_at IS NOT NULL FROM app_files")
            .fetch_all(connection)
            .await?;
    Ok(rows
        .into_iter()
        .map(|(id, created_by_id, deleted)| (id.into_uuid(), created_by_id.into_uuid(), deleted))
        .collect())
}

/// The rows the server wrote after the cursor that the user sees, in
/// cursor order: a page of a pull.
pub async fn list_synced_after(
    connection: &mut SqliteConnection,
    user_id: Uuid,
    after: &Cursor,
    limit: usize,
) -> Result<Vec<File>, sqlx::Error> {
    let rows: Vec<FileRow> = sqlx::query_as(select!(
        "WHERE last_synced_at IS NOT NULL AND (last_synced_at, id) > ($2, $3) AND ",
        visible_to!(),
        " ORDER BY last_synced_at, id LIMIT $4"
    ))
    .bind(user_id.hyphenated())
    .bind(after.last_synced_at)
    .bind(after.id.hyphenated())
    .bind(i64::try_from(limit).unwrap_or(i64::MAX))
    .fetch_all(connection)
    .await?;
    Ok(rows.into_iter().map(File::from).collect())
}

pub async fn insert(connection: &mut SqliteConnection, file: &File) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO app_files (id, created_by_id, name, content_type, size, sha256, inserted_at, updated_at, version, last_synced_at, deleted_at, deleted_by_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
    )
    .bind(file.id.hyphenated())
    .bind(file.created_by_id.hyphenated())
    .bind(&file.name)
    .bind(&file.content_type)
    .bind(size_of(file))
    .bind(&file.sha256)
    .bind(file.inserted_at)
    .bind(file.updated_at)
    .bind(file.version)
    .bind(file.last_synced_at)
    .bind(file.deleted_at)
    .bind(file.deleted_by_id.map(Uuid::hyphenated))
    .execute(connection)
    .await?;
    Ok(())
}

/// Writes what may change over the row; false when there is no such row.
pub async fn update(connection: &mut SqliteConnection, file: &File) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE app_files SET name = $2, updated_at = $3, version = $4, last_synced_at = $5, deleted_at = $6, deleted_by_id = $7
         WHERE id = $1",
    )
    .bind(file.id.hyphenated())
    .bind(&file.name)
    .bind(file.updated_at)
    .bind(file.version)
    .bind(file.last_synced_at)
    .bind(file.deleted_at)
    .bind(file.deleted_by_id.map(Uuid::hyphenated))
    .execute(connection)
    .await?;
    Ok(result.rows_affected() == 1)
}

/// Writes the server's row over the device's, or inserts it.
pub async fn upsert(connection: &mut SqliteConnection, file: &File) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO app_files (id, created_by_id, name, content_type, size, sha256, inserted_at, updated_at, version, last_synced_at, deleted_at, deleted_by_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
         ON CONFLICT (id) DO UPDATE SET
           created_by_id = excluded.created_by_id, name = excluded.name, content_type = excluded.content_type,
           size = excluded.size, sha256 = excluded.sha256, inserted_at = excluded.inserted_at, updated_at = excluded.updated_at,
           version = excluded.version, last_synced_at = excluded.last_synced_at,
           deleted_at = excluded.deleted_at, deleted_by_id = excluded.deleted_by_id",
    )
    .bind(file.id.hyphenated())
    .bind(file.created_by_id.hyphenated())
    .bind(&file.name)
    .bind(&file.content_type)
    .bind(size_of(file))
    .bind(&file.sha256)
    .bind(file.inserted_at)
    .bind(file.updated_at)
    .bind(file.version)
    .bind(file.last_synced_at)
    .bind(file.deleted_at)
    .bind(file.deleted_by_id.map(Uuid::hyphenated))
    .execute(connection)
    .await?;
    Ok(())
}

/// Removes the row itself: a device's file that never reached the
/// server. A delete the person makes of a synced file is a soft delete,
/// see `files::delete`.
pub async fn delete_row(connection: &mut SqliteConnection, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM app_files WHERE id = $1")
        .bind(id.hyphenated())
        .execute(connection)
        .await?;
    Ok(())
}
