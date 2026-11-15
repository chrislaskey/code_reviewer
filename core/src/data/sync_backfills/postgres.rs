//! The `app_sync_backfills` table: the communities a device still has to
//! ask the server for everything in, one row each.

use sqlx::PgConnection;
use uuid::Uuid;

pub async fn insert_if_missing(
    connection: &mut PgConnection,
    community_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO app_sync_backfills (community_id) VALUES ($1) ON CONFLICT (community_id) DO NOTHING")
        .bind(community_id)
        .execute(connection)
        .await?;
    Ok(())
}

pub async fn list(connection: &mut PgConnection) -> Result<Vec<Uuid>, sqlx::Error> {
    let rows: Vec<(Uuid,)> =
        sqlx::query_as("SELECT community_id FROM app_sync_backfills ORDER BY community_id")
            .fetch_all(connection)
            .await?;
    Ok(rows.into_iter().map(|(id,)| id).collect())
}

pub async fn delete(connection: &mut PgConnection, community_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM app_sync_backfills WHERE community_id = $1")
        .bind(community_id)
        .execute(connection)
        .await?;
    Ok(())
}
