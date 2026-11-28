//! The `app_purges` table, and the purge's deletes on every synced
//! table: a row goes once its delete and its last write are both older
//! than the horizon.

use chrono::{DateTime, Utc};
use sqlx::{AssertSqlSafe, SqliteConnection};

use super::Purge;
use crate::database::identifier;

pub async fn delete_deleted_before(
    connection: &mut SqliteConnection,
    table: &str,
    horizon: DateTime<Utc>,
    floor: i64,
) -> Result<u64, sqlx::Error> {
    let table = identifier(table)?;
    let without_dependents = crate::sync::tables::without_dependents(table)?;
    let result = sqlx::query(AssertSqlSafe(format!(
        "DELETE FROM {table} WHERE deleted_at IS NOT NULL AND deleted_at < $1 AND global_sync_version <= $2{without_dependents}"
    )))
    .bind(horizon)
    .bind(floor)
    .execute(connection)
    .await?;
    Ok(result.rows_affected())
}

pub async fn insert(connection: &mut SqliteConnection, purge: &Purge) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO app_purges (id, horizon, inserted_at, minimum_global_sync_version) VALUES ($1, $2, $3, $4)")
        .bind(purge.id.hyphenated())
        .bind(purge.horizon)
        .bind(purge.inserted_at)
        .bind(purge.minimum_global_sync_version)
        .execute(connection)
        .await?;
    Ok(())
}

pub async fn latest_horizon(
    connection: &mut SqliteConnection,
) -> Result<Option<DateTime<Utc>>, sqlx::Error> {
    let row: Option<(DateTime<Utc>,)> =
        sqlx::query_as("SELECT horizon FROM app_purges ORDER BY inserted_at DESC, id DESC LIMIT 1")
            .fetch_optional(connection)
            .await?;
    Ok(row.map(|(horizon,)| horizon))
}
