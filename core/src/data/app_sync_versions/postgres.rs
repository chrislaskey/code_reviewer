//! The `app_sync_versions` table, one row, on PostgreSQL, the
//! counterpart of `sqlite`.

use sqlx::PgConnection;
use uuid::Uuid;

use super::Feed;

pub async fn get(connection: &mut PgConnection) -> Result<Feed, sqlx::Error> {
    let (id, head, floor): (Uuid, i64, i64) = sqlx::query_as(
        "SELECT feed_id, global_sync_version, minimum_global_sync_version
         FROM app_sync_versions WHERE id = 1",
    )
    .fetch_one(connection)
    .await?;
    Ok(Feed { id, head, floor })
}

pub async fn next(connection: &mut PgConnection) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "UPDATE app_sync_versions SET global_sync_version = global_sync_version + 1
         WHERE id = 1 AND global_sync_version < 9223372036854775807
         RETURNING global_sync_version",
    )
    .fetch_one(connection)
    .await
}

pub async fn lock(connection: &mut PgConnection) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT id FROM app_sync_versions WHERE id = 1 FOR UPDATE")
        .fetch_one(connection)
        .await?;
    Ok(())
}

pub async fn update_feed_id(
    connection: &mut PgConnection,
    feed_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE app_sync_versions SET feed_id = $1 WHERE id = 1")
        .bind(feed_id)
        .execute(connection)
        .await?;
    Ok(())
}

pub async fn update_floor(connection: &mut PgConnection, floor: i64) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE app_sync_versions SET minimum_global_sync_version = $1 WHERE id = 1")
        .bind(floor)
        .execute(connection)
        .await?;
    Ok(())
}
