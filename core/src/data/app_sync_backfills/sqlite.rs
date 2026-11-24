//! The `app_sync_backfills` table: the communities a device still has to
//! ask the server for everything in, one row each.

use sqlx::SqliteConnection;
use uuid::Uuid;
use uuid::fmt::Hyphenated;

pub async fn insert_if_missing(
    connection: &mut SqliteConnection,
    community_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO app_sync_backfills (community_id) VALUES ($1) ON CONFLICT (community_id) DO NOTHING")
        .bind(community_id.hyphenated())
        .execute(connection)
        .await?;
    Ok(())
}

pub async fn list(connection: &mut SqliteConnection) -> Result<Vec<Uuid>, sqlx::Error> {
    let rows: Vec<(Hyphenated,)> =
        sqlx::query_as("SELECT community_id FROM app_sync_backfills ORDER BY community_id")
            .fetch_all(connection)
            .await?;
    Ok(rows.into_iter().map(|(id,)| id.into_uuid()).collect())
}

pub async fn delete(
    connection: &mut SqliteConnection,
    community_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM app_sync_backfills WHERE community_id = $1")
        .bind(community_id.hyphenated())
        .execute(connection)
        .await?;
    Ok(())
}
