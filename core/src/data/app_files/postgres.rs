//! The `app_files` table, the counterpart of `sqlite::files` with native
//! uuid and timestamptz columns. A user sees a file in a community they
//! are a live member of.

use chrono::{DateTime, Utc};
use sqlx::{AssertSqlSafe, FromRow, PgConnection};
use uuid::Uuid;

use super::File;
use crate::sync::VersionRange;

#[derive(FromRow)]
struct FileRow {
    id: Uuid,
    created_by_id: Uuid,
    name: String,
    content_type: String,
    size: i64,
    sha256: String,
    content: Option<String>,
    inserted_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    version: i64,
    global_sync_version: Option<i64>,
    deleted_at: Option<DateTime<Utc>>,
    deleted_by_id: Option<Uuid>,
}

impl From<FileRow> for File {
    fn from(row: FileRow) -> File {
        File {
            id: row.id,
            created_by_id: row.created_by_id,
            name: row.name,
            content_type: row.content_type,
            size: u64::try_from(row.size).unwrap_or_default(),
            sha256: row.sha256,
            content: row.content,
            inserted_at: row.inserted_at,
            updated_at: row.updated_at,
            version: row.version,
            global_sync_version: row.global_sync_version,
            deleted_at: row.deleted_at,
            deleted_by_id: row.deleted_by_id,
        }
    }
}

macro_rules! select {
    ($($rest:expr),+ $(,)?) => {
        concat!(
            "SELECT id, created_by_id, name, content_type, size, sha256, content, inserted_at, updated_at, version, global_sync_version, deleted_at, deleted_by_id FROM app_files ",
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

/// A search's files: live, named with `$2` in any case, and of one of
/// the content types `$3` by the type before any `;`.
macro_rules! searched {
    () => {
        "deleted_at IS NULL AND strpos(lower(name), lower($2)) > 0 AND (cardinality($3::text[]) = 0 OR trim(split_part(content_type, ';', 1)) = ANY($3)) AND "
    };
}

fn size_of(file: &File) -> i64 {
    i64::try_from(file.size).unwrap_or(i64::MAX)
}

/// The files the user sees, not deleted, newest first.
pub async fn list_visible(
    connection: &mut PgConnection,
    user_id: Uuid,
) -> Result<Vec<File>, sqlx::Error> {
    let rows: Vec<FileRow> = sqlx::query_as(select!(
        "WHERE deleted_at IS NULL AND ",
        visible_to!(),
        " ORDER BY inserted_at DESC, id DESC"
    ))
    .bind(user_id)
    .fetch_all(connection)
    .await?;
    Ok(rows.into_iter().map(File::from).collect())
}

/// How many files the user sees whose names contain `$2` in any case,
/// of one of the content types `$3` (any for none), not deleted, and
/// `limit` of them from `offset`, newest first.
pub async fn search_visible(
    connection: &mut PgConnection,
    user_id: Uuid,
    name: &str,
    content_types: &[String],
    offset: i64,
    limit: i64,
) -> Result<(u64, Vec<File>), sqlx::Error> {
    let (total,): (i64,) = sqlx::query_as(concat!(
        "SELECT COUNT(*) FROM app_files WHERE ",
        searched!(),
        visible_to!()
    ))
    .bind(user_id)
    .bind(name)
    .bind(content_types)
    .fetch_one(&mut *connection)
    .await?;
    let rows: Vec<FileRow> = sqlx::query_as(select!(
        "WHERE ",
        searched!(),
        visible_to!(),
        " ORDER BY inserted_at DESC, id DESC LIMIT $4 OFFSET $5"
    ))
    .bind(user_id)
    .bind(name)
    .bind(content_types)
    .bind(limit)
    .bind(offset)
    .fetch_all(connection)
    .await?;
    Ok((
        u64::try_from(total).unwrap_or_default(),
        rows.into_iter().map(File::from).collect(),
    ))
}

/// The files the user sees with any of these checksums, not deleted,
/// newest first.
pub async fn list_visible_with_sha256s(
    connection: &mut PgConnection,
    user_id: Uuid,
    sha256s: &[String],
) -> Result<Vec<File>, sqlx::Error> {
    let rows: Vec<FileRow> = sqlx::query_as(select!(
        "WHERE deleted_at IS NULL AND sha256 = ANY($2) AND ",
        visible_to!(),
        " ORDER BY inserted_at DESC, id DESC"
    ))
    .bind(user_id)
    .bind(sha256s)
    .fetch_all(connection)
    .await?;
    Ok(rows.into_iter().map(File::from).collect())
}

/// The file by its id if the user sees it, not deleted.
pub async fn get_visible(
    connection: &mut PgConnection,
    user_id: Uuid,
    id: Uuid,
) -> Result<Option<File>, sqlx::Error> {
    let row: Option<FileRow> = sqlx::query_as(select!(
        "WHERE id = $2 AND deleted_at IS NULL AND ",
        visible_to!()
    ))
    .bind(user_id)
    .bind(id)
    .fetch_optional(connection)
    .await?;
    Ok(row.map(File::from))
}

/// The ids of the files on this database the user does not see: what a
/// device prunes.
pub async fn list_invisible_ids(
    connection: &mut PgConnection,
    user_id: Uuid,
) -> Result<Vec<Uuid>, sqlx::Error> {
    let rows: Vec<(Uuid,)> = sqlx::query_as(concat!(
        "SELECT id FROM app_files WHERE NOT ",
        visible_to!()
    ))
    .bind(user_id)
    .fetch_all(connection)
    .await?;
    Ok(rows.into_iter().map(|(id,)| id).collect())
}

/// The row by its id, deleted or not, whoever sees it.
pub async fn get(connection: &mut PgConnection, id: Uuid) -> Result<Option<File>, sqlx::Error> {
    let row: Option<FileRow> = sqlx::query_as(select!("WHERE id = $1"))
        .bind(id)
        .fetch_optional(connection)
        .await?;
    Ok(row.map(File::from))
}

/// Every row's id and creator, with whether it is deleted: what the
/// bytes kept for the rows are checked against.
pub async fn list_ids(
    connection: &mut PgConnection,
) -> Result<Vec<(Uuid, Uuid, bool)>, sqlx::Error> {
    sqlx::query_as("SELECT id, created_by_id, deleted_at IS NOT NULL FROM app_files")
        .fetch_all(connection)
        .await
}

/// The rows the server wrote after the cursor that the user sees, in
/// cursor order: a page of a pull.
pub async fn list_synced_after(
    connection: &mut PgConnection,
    user_id: Uuid,
    range: &VersionRange,
    limit: usize,
) -> Result<Vec<File>, sqlx::Error> {
    let rows: Vec<FileRow> = sqlx::query_as(select!(
        "WHERE global_sync_version > $2 AND global_sync_version <= $3 AND ",
        visible_to!(),
        " ORDER BY global_sync_version LIMIT $4"
    ))
    .bind(user_id)
    .bind(range.after)
    .bind(range.through)
    .bind(i64::try_from(limit).unwrap_or(i64::MAX))
    .fetch_all(connection)
    .await?;
    Ok(rows.into_iter().map(File::from).collect())
}

pub async fn insert(connection: &mut PgConnection, file: &File) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO app_files (id, created_by_id, name, content_type, size, sha256, content, inserted_at, updated_at, version, global_sync_version, deleted_at, deleted_by_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)",
    )
    .bind(file.id)
    .bind(file.created_by_id)
    .bind(&file.name)
    .bind(&file.content_type)
    .bind(size_of(file))
    .bind(&file.sha256)
    .bind(&file.content)
    .bind(file.inserted_at)
    .bind(file.updated_at)
    .bind(file.version)
    .bind(file.global_sync_version)
    .bind(file.deleted_at)
    .bind(file.deleted_by_id)
    .execute(connection)
    .await?;
    Ok(())
}

/// Writes what may change over the row; false when there is no such row.
pub async fn update(connection: &mut PgConnection, file: &File) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE app_files SET name = $2, updated_at = $3, version = $4, global_sync_version = $5, deleted_at = $6, deleted_by_id = $7
         WHERE id = $1",
    )
    .bind(file.id)
    .bind(&file.name)
    .bind(file.updated_at)
    .bind(file.version)
    .bind(file.global_sync_version)
    .bind(file.deleted_at)
    .bind(file.deleted_by_id)
    .execute(connection)
    .await?;
    Ok(result.rows_affected() == 1)
}

/// Writes the server's row over the device's, or inserts it.
pub async fn upsert(connection: &mut PgConnection, file: &File) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO app_files (id, created_by_id, name, content_type, size, sha256, content, inserted_at, updated_at, version, global_sync_version, deleted_at, deleted_by_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
         ON CONFLICT (id) DO UPDATE SET
           created_by_id = excluded.created_by_id, name = excluded.name, content_type = excluded.content_type,
           size = excluded.size, sha256 = excluded.sha256, content = excluded.content, inserted_at = excluded.inserted_at, updated_at = excluded.updated_at,
           version = excluded.version, global_sync_version = excluded.global_sync_version,
           deleted_at = excluded.deleted_at, deleted_by_id = excluded.deleted_by_id",
    )
    .bind(file.id)
    .bind(file.created_by_id)
    .bind(&file.name)
    .bind(&file.content_type)
    .bind(size_of(file))
    .bind(&file.sha256)
    .bind(&file.content)
    .bind(file.inserted_at)
    .bind(file.updated_at)
    .bind(file.version)
    .bind(file.global_sync_version)
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

/// Drops the files for a resync from nothing, but those an event still
/// pending is about, and those whose create the server refused: a
/// refused file is the only copy of its bytes.
pub async fn delete_for_resync(connection: &mut PgConnection) -> Result<(), sqlx::Error> {
    let without_dependents = crate::sync::tables::without_dependents("app_files")?;
    sqlx::query(AssertSqlSafe(format!(
        "DELETE FROM app_files WHERE id NOT IN
           (SELECT row_id FROM app_events WHERE table_name = 'app_files'
            AND (status = 'pending' OR (status = 'conflict' AND type = 'create'))){without_dependents}"
    )))
    .execute(connection)
    .await?;
    Ok(())
}
