//! The `app_files_communities` table: a file in a community, see
//! `files_communities`.

use chrono::{DateTime, Utc};
use sqlx::types::Json;
use sqlx::{AssertSqlSafe, FromRow, SqliteConnection};
use uuid::Uuid;
use uuid::fmt::Hyphenated;

use super::FileCommunity;
use crate::sync::VersionRange;

#[derive(FromRow)]
struct FileCommunityRow {
    id: Hyphenated,
    file_id: Hyphenated,
    community_id: Hyphenated,
    created_by_id: Hyphenated,
    inserted_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    version: i64,
    global_sync_version: Option<i64>,
    deleted_at: Option<DateTime<Utc>>,
    deleted_by_id: Option<Hyphenated>,
}

impl From<FileCommunityRow> for FileCommunity {
    fn from(row: FileCommunityRow) -> FileCommunity {
        FileCommunity {
            id: row.id.into_uuid(),
            file_id: row.file_id.into_uuid(),
            community_id: row.community_id.into_uuid(),
            created_by_id: row.created_by_id.into_uuid(),
            inserted_at: row.inserted_at,
            updated_at: row.updated_at,
            version: row.version,
            global_sync_version: row.global_sync_version,
            deleted_at: row.deleted_at,
            deleted_by_id: row.deleted_by_id.map(Hyphenated::into_uuid),
        }
    }
}

macro_rules! select {
    ($($rest:expr),+ $(,)?) => {
        concat!(
            "SELECT id, file_id, community_id, created_by_id, inserted_at, updated_at, version, global_sync_version, deleted_at, deleted_by_id FROM app_files_communities ",
            $($rest),+
        )
    };
}

/// The rows the user, `$1`, sees: those in a community they are a live
/// member of, and no other, so nothing tells them a file is elsewhere.
macro_rules! visible_to {
    () => {
        "community_id IN (SELECT community_id FROM app_users_communities WHERE user_id = $1 AND deleted_at IS NULL)"
    };
}

fn ids_json(ids: &[Uuid]) -> Json<Vec<String>> {
    Json(ids.iter().map(|id| id.hyphenated().to_string()).collect())
}

/// The row by its id, deleted or not.
pub async fn get(
    connection: &mut SqliteConnection,
    id: Uuid,
) -> Result<Option<FileCommunity>, sqlx::Error> {
    let row: Option<FileCommunityRow> = sqlx::query_as(select!("WHERE id = $1"))
        .bind(id.hyphenated())
        .fetch_optional(connection)
        .await?;
    Ok(row.map(FileCommunity::from))
}

/// The file's row for the community, deleted or not: there is one at
/// most, revived by putting the file back.
pub async fn get_by_file_and_community(
    connection: &mut SqliteConnection,
    file_id: Uuid,
    community_id: Uuid,
) -> Result<Option<FileCommunity>, sqlx::Error> {
    let row: Option<FileCommunityRow> =
        sqlx::query_as(select!("WHERE file_id = $1 AND community_id = $2"))
            .bind(file_id.hyphenated())
            .bind(community_id.hyphenated())
            .fetch_optional(connection)
            .await?;
    Ok(row.map(FileCommunity::from))
}

/// The live rows of the files, in every community.
pub async fn list_live_of_files(
    connection: &mut SqliteConnection,
    ids: &[Uuid],
) -> Result<Vec<FileCommunity>, sqlx::Error> {
    let rows: Vec<FileCommunityRow> = sqlx::query_as(select!(
        "WHERE deleted_at IS NULL AND file_id IN (SELECT value FROM json_each($1)) ORDER BY inserted_at, id"
    ))
    .bind(ids_json(ids))
    .fetch_all(connection)
    .await?;
    Ok(rows.into_iter().map(FileCommunity::from).collect())
}

/// The live rows of the files the user sees.
pub async fn list_live_visible_of_files(
    connection: &mut SqliteConnection,
    user_id: Uuid,
    ids: &[Uuid],
) -> Result<Vec<FileCommunity>, sqlx::Error> {
    let rows: Vec<FileCommunityRow> = sqlx::query_as(select!(
        "WHERE deleted_at IS NULL AND file_id IN (SELECT value FROM json_each($2)) AND ",
        visible_to!(),
        " ORDER BY inserted_at, id"
    ))
    .bind(user_id.hyphenated())
    .bind(ids_json(ids))
    .fetch_all(connection)
    .await?;
    Ok(rows.into_iter().map(FileCommunity::from).collect())
}

/// The live rows in the community.
pub async fn list_live_of_community(
    connection: &mut SqliteConnection,
    community_id: Uuid,
) -> Result<Vec<FileCommunity>, sqlx::Error> {
    let rows: Vec<FileCommunityRow> = sqlx::query_as(select!(
        "WHERE community_id = $1 AND deleted_at IS NULL ORDER BY inserted_at, id"
    ))
    .bind(community_id.hyphenated())
    .fetch_all(connection)
    .await?;
    Ok(rows.into_iter().map(FileCommunity::from).collect())
}

/// The rows the server wrote after the cursor that the user sees, in
/// cursor order, deleted or not: a page of a pull.
pub async fn list_synced_after(
    connection: &mut SqliteConnection,
    user_id: Uuid,
    range: &VersionRange,
    limit: usize,
) -> Result<Vec<FileCommunity>, sqlx::Error> {
    let rows: Vec<FileCommunityRow> = sqlx::query_as(select!(
        "WHERE global_sync_version > $2 AND global_sync_version <= $3 AND ",
        visible_to!(),
        " ORDER BY global_sync_version LIMIT $4"
    ))
    .bind(user_id.hyphenated())
    .bind(range.after)
    .bind(range.through)
    .bind(i64::try_from(limit).unwrap_or(i64::MAX))
    .fetch_all(connection)
    .await?;
    Ok(rows.into_iter().map(FileCommunity::from).collect())
}

/// The live rows of the community after the cursor, in cursor order: a
/// page of a backfill.
pub async fn list_live_of_community_after(
    connection: &mut SqliteConnection,
    community_id: Uuid,
    range: &VersionRange,
    limit: usize,
) -> Result<Vec<FileCommunity>, sqlx::Error> {
    let rows: Vec<FileCommunityRow> = sqlx::query_as(select!(
        "WHERE community_id = $1 AND deleted_at IS NULL
           AND global_sync_version > $2 AND global_sync_version <= $3
         ORDER BY global_sync_version LIMIT $4"
    ))
    .bind(community_id.hyphenated())
    .bind(range.after)
    .bind(range.through)
    .bind(i64::try_from(limit).unwrap_or(i64::MAX))
    .fetch_all(connection)
    .await?;
    Ok(rows.into_iter().map(FileCommunity::from).collect())
}

pub async fn insert(
    connection: &mut SqliteConnection,
    row: &FileCommunity,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO app_files_communities (id, file_id, community_id, created_by_id, inserted_at, updated_at, version, global_sync_version, deleted_at, deleted_by_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
    )
    .bind(row.id.hyphenated())
    .bind(row.file_id.hyphenated())
    .bind(row.community_id.hyphenated())
    .bind(row.created_by_id.hyphenated())
    .bind(row.inserted_at)
    .bind(row.updated_at)
    .bind(row.version)
    .bind(row.global_sync_version)
    .bind(row.deleted_at)
    .bind(row.deleted_by_id.map(Uuid::hyphenated))
    .execute(connection)
    .await?;
    Ok(())
}

/// Writes the row over itself; false when there is no such row.
pub async fn update(
    connection: &mut SqliteConnection,
    row: &FileCommunity,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE app_files_communities SET updated_at = $2, version = $3, global_sync_version = $4, deleted_at = $5, deleted_by_id = $6
         WHERE id = $1",
    )
    .bind(row.id.hyphenated())
    .bind(row.updated_at)
    .bind(row.version)
    .bind(row.global_sync_version)
    .bind(row.deleted_at)
    .bind(row.deleted_by_id.map(Uuid::hyphenated))
    .execute(connection)
    .await?;
    Ok(result.rows_affected() == 1)
}

/// Writes the server's row over the device's, or inserts it.
pub async fn upsert(
    connection: &mut SqliteConnection,
    row: &FileCommunity,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO app_files_communities (id, file_id, community_id, created_by_id, inserted_at, updated_at, version, global_sync_version, deleted_at, deleted_by_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
         ON CONFLICT (id) DO UPDATE SET
           file_id = excluded.file_id, community_id = excluded.community_id, created_by_id = excluded.created_by_id,
           inserted_at = excluded.inserted_at, updated_at = excluded.updated_at, version = excluded.version,
           global_sync_version = excluded.global_sync_version, deleted_at = excluded.deleted_at, deleted_by_id = excluded.deleted_by_id",
    )
    .bind(row.id.hyphenated())
    .bind(row.file_id.hyphenated())
    .bind(row.community_id.hyphenated())
    .bind(row.created_by_id.hyphenated())
    .bind(row.inserted_at)
    .bind(row.updated_at)
    .bind(row.version)
    .bind(row.global_sync_version)
    .bind(row.deleted_at)
    .bind(row.deleted_by_id.map(Uuid::hyphenated))
    .execute(connection)
    .await?;
    Ok(())
}

/// Removes the row itself, when a device's row was replaced by the
/// server's under another id, or its create refused.
pub async fn delete_row(connection: &mut SqliteConnection, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM app_files_communities WHERE id = $1")
        .bind(id.hyphenated())
        .execute(connection)
        .await?;
    Ok(())
}

/// Drops the rows for a resync from nothing, but those an event still
/// pending is about, and those of a file whose create is pending or was
/// refused, which came with that create.
pub async fn delete_for_resync(connection: &mut SqliteConnection) -> Result<(), sqlx::Error> {
    let without_dependents = crate::sync::tables::without_dependents("app_files_communities")?;
    sqlx::query(AssertSqlSafe(format!(
        "DELETE FROM app_files_communities WHERE id NOT IN
           (SELECT row_id FROM app_events WHERE status = 'pending' AND table_name = 'app_files_communities')
           AND file_id NOT IN (SELECT row_id FROM app_events WHERE table_name = 'app_files'
            AND type = 'create' AND status IN ('pending', 'conflict')){without_dependents}"
    )))
    .execute(connection)
    .await?;
    Ok(())
}

pub async fn move_rows_to_community(
    connection: &mut SqliteConnection,
    from_community_id: Uuid,
    to_community_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "DELETE FROM app_files_communities WHERE community_id = $1 \
         AND file_id IN (SELECT file_id FROM app_files_communities WHERE community_id = $2)",
    )
    .bind(from_community_id.hyphenated())
    .bind(to_community_id.hyphenated())
    .execute(&mut *connection)
    .await?;
    sqlx::query("UPDATE app_files_communities SET community_id = $2 WHERE community_id = $1")
        .bind(from_community_id.hyphenated())
        .bind(to_community_id.hyphenated())
        .execute(connection)
        .await?;
    Ok(())
}
