//! The `app_files_communities` table, the counterpart of
//! `sqlite::files_communities` with native uuid and timestamptz columns:
//! a file in a community, see `files_communities`.

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgConnection};
use uuid::Uuid;

use super::FileCommunity;
use crate::app::sync::Cursor;

#[derive(FromRow)]
struct FileCommunityRow {
    id: Uuid,
    file_id: Uuid,
    community_id: Uuid,
    created_by_id: Uuid,
    inserted_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    version: i64,
    last_synced_at: Option<DateTime<Utc>>,
    deleted_at: Option<DateTime<Utc>>,
    deleted_by_id: Option<Uuid>,
}

impl From<FileCommunityRow> for FileCommunity {
    fn from(row: FileCommunityRow) -> FileCommunity {
        FileCommunity {
            id: row.id,
            file_id: row.file_id,
            community_id: row.community_id,
            created_by_id: row.created_by_id,
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
    ($($rest:expr),+ $(,)?) => {
        concat!(
            "SELECT id, file_id, community_id, created_by_id, inserted_at, updated_at, version, last_synced_at, deleted_at, deleted_by_id FROM app_files_communities ",
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

/// The row by its id, deleted or not.
pub async fn get(
    connection: &mut PgConnection,
    id: Uuid,
) -> Result<Option<FileCommunity>, sqlx::Error> {
    let row: Option<FileCommunityRow> = sqlx::query_as(select!("WHERE id = $1"))
        .bind(id)
        .fetch_optional(connection)
        .await?;
    Ok(row.map(FileCommunity::from))
}

/// The file's row for the community, deleted or not: there is one at
/// most, revived by putting the file back.
pub async fn get_by_file_and_community(
    connection: &mut PgConnection,
    file_id: Uuid,
    community_id: Uuid,
) -> Result<Option<FileCommunity>, sqlx::Error> {
    let row: Option<FileCommunityRow> =
        sqlx::query_as(select!("WHERE file_id = $1 AND community_id = $2"))
            .bind(file_id)
            .bind(community_id)
            .fetch_optional(connection)
            .await?;
    Ok(row.map(FileCommunity::from))
}

/// The live rows of the files, in every community.
pub async fn list_live_of_files(
    connection: &mut PgConnection,
    ids: &[Uuid],
) -> Result<Vec<FileCommunity>, sqlx::Error> {
    let rows: Vec<FileCommunityRow> = sqlx::query_as(select!(
        "WHERE deleted_at IS NULL AND file_id = ANY($1) ORDER BY inserted_at, id"
    ))
    .bind(ids)
    .fetch_all(connection)
    .await?;
    Ok(rows.into_iter().map(FileCommunity::from).collect())
}

/// The live rows of the files the user sees.
pub async fn list_live_visible_of_files(
    connection: &mut PgConnection,
    user_id: Uuid,
    ids: &[Uuid],
) -> Result<Vec<FileCommunity>, sqlx::Error> {
    let rows: Vec<FileCommunityRow> = sqlx::query_as(select!(
        "WHERE deleted_at IS NULL AND file_id = ANY($2) AND ",
        visible_to!(),
        " ORDER BY inserted_at, id"
    ))
    .bind(user_id)
    .bind(ids)
    .fetch_all(connection)
    .await?;
    Ok(rows.into_iter().map(FileCommunity::from).collect())
}

/// The live rows in the community.
pub async fn list_live_of_community(
    connection: &mut PgConnection,
    community_id: Uuid,
) -> Result<Vec<FileCommunity>, sqlx::Error> {
    let rows: Vec<FileCommunityRow> = sqlx::query_as(select!(
        "WHERE community_id = $1 AND deleted_at IS NULL ORDER BY inserted_at, id"
    ))
    .bind(community_id)
    .fetch_all(connection)
    .await?;
    Ok(rows.into_iter().map(FileCommunity::from).collect())
}

/// The rows the server wrote after the cursor that the user sees, in
/// cursor order, deleted or not: a page of a pull.
pub async fn list_synced_after(
    connection: &mut PgConnection,
    user_id: Uuid,
    after: &Cursor,
    limit: usize,
) -> Result<Vec<FileCommunity>, sqlx::Error> {
    let rows: Vec<FileCommunityRow> = sqlx::query_as(select!(
        "WHERE last_synced_at IS NOT NULL AND (last_synced_at, id) > ($2, $3) AND ",
        visible_to!(),
        " ORDER BY last_synced_at, id LIMIT $4"
    ))
    .bind(user_id)
    .bind(after.last_synced_at)
    .bind(after.id)
    .bind(i64::try_from(limit).unwrap_or(i64::MAX))
    .fetch_all(connection)
    .await?;
    Ok(rows.into_iter().map(FileCommunity::from).collect())
}

/// The live rows of the community after the cursor, in cursor order: a
/// page of a backfill.
pub async fn list_live_of_community_after(
    connection: &mut PgConnection,
    community_id: Uuid,
    after: &Cursor,
    limit: usize,
) -> Result<Vec<FileCommunity>, sqlx::Error> {
    let rows: Vec<FileCommunityRow> = sqlx::query_as(select!(
        "WHERE community_id = $1 AND deleted_at IS NULL AND last_synced_at IS NOT NULL
           AND (last_synced_at, id) > ($2, $3)
         ORDER BY last_synced_at, id LIMIT $4"
    ))
    .bind(community_id)
    .bind(after.last_synced_at)
    .bind(after.id)
    .bind(i64::try_from(limit).unwrap_or(i64::MAX))
    .fetch_all(connection)
    .await?;
    Ok(rows.into_iter().map(FileCommunity::from).collect())
}

pub async fn insert(connection: &mut PgConnection, row: &FileCommunity) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO app_files_communities (id, file_id, community_id, created_by_id, inserted_at, updated_at, version, last_synced_at, deleted_at, deleted_by_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
    )
    .bind(row.id)
    .bind(row.file_id)
    .bind(row.community_id)
    .bind(row.created_by_id)
    .bind(row.inserted_at)
    .bind(row.updated_at)
    .bind(row.version)
    .bind(row.last_synced_at)
    .bind(row.deleted_at)
    .bind(row.deleted_by_id)
    .execute(connection)
    .await?;
    Ok(())
}

/// Writes the row over itself; false when there is no such row.
pub async fn update(
    connection: &mut PgConnection,
    row: &FileCommunity,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE app_files_communities SET updated_at = $2, version = $3, last_synced_at = $4, deleted_at = $5, deleted_by_id = $6
         WHERE id = $1",
    )
    .bind(row.id)
    .bind(row.updated_at)
    .bind(row.version)
    .bind(row.last_synced_at)
    .bind(row.deleted_at)
    .bind(row.deleted_by_id)
    .execute(connection)
    .await?;
    Ok(result.rows_affected() == 1)
}

/// Writes the server's row over the device's, or inserts it.
pub async fn upsert(connection: &mut PgConnection, row: &FileCommunity) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO app_files_communities (id, file_id, community_id, created_by_id, inserted_at, updated_at, version, last_synced_at, deleted_at, deleted_by_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
         ON CONFLICT (id) DO UPDATE SET
           file_id = excluded.file_id, community_id = excluded.community_id, created_by_id = excluded.created_by_id,
           inserted_at = excluded.inserted_at, updated_at = excluded.updated_at, version = excluded.version,
           last_synced_at = excluded.last_synced_at, deleted_at = excluded.deleted_at, deleted_by_id = excluded.deleted_by_id",
    )
    .bind(row.id)
    .bind(row.file_id)
    .bind(row.community_id)
    .bind(row.created_by_id)
    .bind(row.inserted_at)
    .bind(row.updated_at)
    .bind(row.version)
    .bind(row.last_synced_at)
    .bind(row.deleted_at)
    .bind(row.deleted_by_id)
    .execute(connection)
    .await?;
    Ok(())
}

/// Removes the row itself, when a device's row was replaced by the
/// server's under another id, or its create refused.
pub async fn delete_row(connection: &mut PgConnection, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM app_files_communities WHERE id = $1")
        .bind(id)
        .execute(connection)
        .await?;
    Ok(())
}
