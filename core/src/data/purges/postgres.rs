//! The `app_purges` table, and the purge's deletes on every synced
//! table: a row goes once its delete and its last write are both older
//! than the horizon.

use chrono::{DateTime, Utc};
use sqlx::PgConnection;

use crate::purge::Purge;

pub async fn delete_deleted_before(
    connection: &mut PgConnection,
    table: &str,
    horizon: DateTime<Utc>,
) -> Result<u64, sqlx::Error> {
    let statement = match table {
        "app_posts_communities" => {
            "DELETE FROM app_posts_communities WHERE deleted_at IS NOT NULL AND deleted_at < $1 AND last_synced_at < $1"
        }
        "app_files" => {
            "DELETE FROM app_files WHERE deleted_at IS NOT NULL AND deleted_at < $1 AND last_synced_at < $1"
        }
        "app_posts" => {
            "DELETE FROM app_posts WHERE deleted_at IS NOT NULL AND deleted_at < $1 AND last_synced_at < $1"
        }
        "app_users_communities" => {
            "DELETE FROM app_users_communities WHERE deleted_at IS NOT NULL AND deleted_at < $1 AND last_synced_at < $1"
        }
        "app_communities" => {
            "DELETE FROM app_communities WHERE deleted_at IS NOT NULL AND deleted_at < $1 AND last_synced_at < $1"
        }
        "app_users" => {
            "DELETE FROM app_users WHERE deleted_at IS NOT NULL AND deleted_at < $1 AND last_synced_at < $1"
        }
        other => return Err(sqlx::Error::Protocol(format!("no purge of {other}"))),
    };
    let result = sqlx::query(statement)
        .bind(horizon)
        .execute(connection)
        .await?;
    Ok(result.rows_affected())
}

pub async fn insert(connection: &mut PgConnection, purge: &Purge) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO app_purges (id, horizon, inserted_at) VALUES ($1, $2, $3)")
        .bind(purge.id)
        .bind(purge.horizon)
        .bind(purge.inserted_at)
        .execute(connection)
        .await?;
    Ok(())
}

pub async fn latest_horizon(
    connection: &mut PgConnection,
) -> Result<Option<DateTime<Utc>>, sqlx::Error> {
    let row: Option<(DateTime<Utc>,)> =
        sqlx::query_as("SELECT horizon FROM app_purges ORDER BY inserted_at DESC, id DESC LIMIT 1")
            .fetch_optional(connection)
            .await?;
    Ok(row.map(|(horizon,)| horizon))
}
