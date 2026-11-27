//! The `app_sync_versions` table, one row, on SQLite.

use sqlx::SqliteConnection;
use uuid::fmt::Hyphenated;

use super::Feed;

pub async fn get(connection: &mut SqliteConnection) -> Result<Feed, sqlx::Error> {
    let (id, head, floor): (Hyphenated, i64, i64) = sqlx::query_as(
        "SELECT feed_id, global_sync_version, minimum_global_sync_version
         FROM app_sync_versions WHERE id = 1",
    )
    .fetch_one(connection)
    .await?;
    Ok(Feed {
        id: id.into_uuid(),
        head,
        floor,
    })
}

pub async fn next(connection: &mut SqliteConnection) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "UPDATE app_sync_versions SET global_sync_version = global_sync_version + 1
         WHERE id = 1 AND global_sync_version < 9223372036854775807
         RETURNING global_sync_version",
    )
    .fetch_one(connection)
    .await
}

/// A SQLite write transaction is the only writer already.
pub async fn lock(_connection: &mut SqliteConnection) -> Result<(), sqlx::Error> {
    Ok(())
}

pub async fn update_floor(
    connection: &mut SqliteConnection,
    floor: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE app_sync_versions SET minimum_global_sync_version = $1 WHERE id = 1")
        .bind(floor)
        .execute(connection)
        .await?;
    Ok(())
}
