//! The `app_files` table, the counterpart of `sqlite::files` with native
//! uuid and timestamptz columns. A user sees the files they own.

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgConnection};
use uuid::Uuid;

use crate::files::File;
use crate::sync::Cursor;

#[derive(FromRow)]
struct FileRow {
    id: Uuid,
    user_id: Uuid,
    name: String,
    content_type: String,
    size: i64,
    inserted_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    version: i64,
    last_synced_at: Option<DateTime<Utc>>,
    deleted_at: Option<DateTime<Utc>>,
    deleted_by_id: Option<Uuid>,
}

impl From<FileRow> for File {
    fn from(row: FileRow) -> File {
        File {
            id: row.id,
            user_id: row.user_id,
            name: row.name,
            content_type: row.content_type,
            size: u64::try_from(row.size).unwrap_or_default(),
            inserted_at: row.inserted_at,
            updated_at: row.updated_at,
            version: row.version,
            last_synced_at: row.last_synced_at,
            deleted_at: row.deleted_at,
            deleted_by_id: row.deleted_by_id,
        }
    }
}

macro_rules! select {
    ($rest:literal) => {
        concat!(
            "SELECT id, user_id, name, content_type, size, inserted_at, updated_at, version, last_synced_at, deleted_at, deleted_by_id FROM app_files ",
            $rest
        )
    };
}

fn size_of(file: &File) -> i64 {
    i64::try_from(file.size).unwrap_or(i64::MAX)
}

/// The user's files, not deleted, newest first.
pub async fn list_owned(
    connection: &mut PgConnection,
    user_id: Uuid,
) -> Result<Vec<File>, sqlx::Error> {
    let rows: Vec<FileRow> = sqlx::query_as(select!(
        "WHERE deleted_at IS NULL AND user_id = $1 ORDER BY inserted_at DESC, id DESC"
    ))
    .bind(user_id)
    .fetch_all(connection)
    .await?;
    Ok(rows.into_iter().map(File::from).collect())
}

/// The row by its id, deleted or not, whoever owns it.
pub async fn get(connection: &mut PgConnection, id: Uuid) -> Result<Option<File>, sqlx::Error> {
    let row: Option<FileRow> = sqlx::query_as(select!("WHERE id = $1"))
        .bind(id)
        .fetch_optional(connection)
        .await?;
    Ok(row.map(File::from))
}

/// Every row's id and owner, with whether it is deleted: what the
/// bytes kept for the rows are checked against.
pub async fn list_ids(
    connection: &mut PgConnection,
) -> Result<Vec<(Uuid, Uuid, bool)>, sqlx::Error> {
    sqlx::query_as("SELECT id, user_id, deleted_at IS NOT NULL FROM app_files")
        .fetch_all(connection)
        .await
}

/// The user's rows the server wrote after the cursor, in cursor order,
/// deleted or not: a page of a pull.
pub async fn list_synced_after(
    connection: &mut PgConnection,
    user_id: Uuid,
    after: &Cursor,
    limit: usize,
) -> Result<Vec<File>, sqlx::Error> {
    let rows: Vec<FileRow> = sqlx::query_as(select!(
        "WHERE last_synced_at IS NOT NULL AND (last_synced_at, id) > ($2, $3) AND user_id = $1
         ORDER BY last_synced_at, id LIMIT $4"
    ))
    .bind(user_id)
    .bind(after.last_synced_at)
    .bind(after.id)
    .bind(i64::try_from(limit).unwrap_or(i64::MAX))
    .fetch_all(connection)
    .await?;
    Ok(rows.into_iter().map(File::from).collect())
}

pub async fn insert(connection: &mut PgConnection, file: &File) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO app_files (id, user_id, name, content_type, size, inserted_at, updated_at, version, last_synced_at, deleted_at, deleted_by_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
    )
    .bind(file.id)
    .bind(file.user_id)
    .bind(&file.name)
    .bind(&file.content_type)
    .bind(size_of(file))
    .bind(file.inserted_at)
    .bind(file.updated_at)
    .bind(file.version)
    .bind(file.last_synced_at)
    .bind(file.deleted_at)
    .bind(file.deleted_by_id)
    .execute(connection)
    .await?;
    Ok(())
}

/// Writes what may change over the row; false when there is no such row.
pub async fn update(connection: &mut PgConnection, file: &File) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE app_files SET name = $2, updated_at = $3, version = $4, last_synced_at = $5, deleted_at = $6, deleted_by_id = $7
         WHERE id = $1",
    )
    .bind(file.id)
    .bind(&file.name)
    .bind(file.updated_at)
    .bind(file.version)
    .bind(file.last_synced_at)
    .bind(file.deleted_at)
    .bind(file.deleted_by_id)
    .execute(connection)
    .await?;
    Ok(result.rows_affected() == 1)
}

/// Writes the server's row over the device's, or inserts it.
pub async fn upsert(connection: &mut PgConnection, file: &File) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO app_files (id, user_id, name, content_type, size, inserted_at, updated_at, version, last_synced_at, deleted_at, deleted_by_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
         ON CONFLICT (id) DO UPDATE SET
           user_id = excluded.user_id, name = excluded.name, content_type = excluded.content_type,
           size = excluded.size, inserted_at = excluded.inserted_at, updated_at = excluded.updated_at,
           version = excluded.version, last_synced_at = excluded.last_synced_at,
           deleted_at = excluded.deleted_at, deleted_by_id = excluded.deleted_by_id",
    )
    .bind(file.id)
    .bind(file.user_id)
    .bind(&file.name)
    .bind(&file.content_type)
    .bind(size_of(file))
    .bind(file.inserted_at)
    .bind(file.updated_at)
    .bind(file.version)
    .bind(file.last_synced_at)
    .bind(file.deleted_at)
    .bind(file.deleted_by_id)
    .execute(connection)
    .await?;
    Ok(())
}

/// Removes the row itself: a device's file that never reached the
/// server. A delete the person makes of a synced file is a soft delete,
/// see `files::delete`.
pub async fn delete_row(connection: &mut PgConnection, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM app_files WHERE id = $1")
        .bind(id)
        .execute(connection)
        .await?;
    Ok(())
}
