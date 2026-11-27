//! The `app_sync_changes` table on SQLite.

use chrono::{DateTime, Utc};
use sqlx::SqliteConnection;
use uuid::Uuid;

pub async fn insert(
    connection: &mut SqliteConnection,
    version: i64,
    table: &str,
    row_id: Uuid,
    written_at: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO app_sync_changes (global_sync_version, table_name, row_id, written_at)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(version)
    .bind(table)
    .bind(row_id.hyphenated())
    .bind(written_at)
    .execute(connection)
    .await?;
    Ok(())
}

pub async fn first_written_at_or_after(
    connection: &mut SqliteConnection,
    horizon: DateTime<Utc>,
) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT MIN(global_sync_version) FROM app_sync_changes WHERE written_at >= $1",
    )
    .bind(horizon)
    .fetch_one(connection)
    .await
}

pub async fn delete_through(
    connection: &mut SqliteConnection,
    version: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM app_sync_changes WHERE global_sync_version <= $1")
        .bind(version)
        .execute(connection)
        .await?;
    Ok(())
}

#[cfg(test)]
pub async fn list_unpublished(
    connection: &mut SqliteConnection,
    table: &str,
) -> Result<Vec<String>, sqlx::Error> {
    let name = crate::database::identifier(table)?;
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT CAST(t.id AS TEXT) FROM {name} t
         WHERE t.global_sync_version IS NULL
            OR (t.global_sync_version >
                  (SELECT minimum_global_sync_version FROM app_sync_versions WHERE id = 1)
                AND NOT EXISTS (SELECT 1 FROM app_sync_changes c
                  WHERE c.global_sync_version = t.global_sync_version
                    AND c.table_name = $1 AND c.row_id = t.id))
            OR EXISTS (SELECT 1 FROM app_sync_changes c
                  WHERE c.table_name = $1 AND c.row_id = t.id
                    AND c.global_sync_version > t.global_sync_version)"
    )))
    .bind(table)
    .fetch_all(connection)
    .await
}
