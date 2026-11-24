//! The `app_sync_state` table: a device's pull cursor, one row, written by
//! the first pull.

use chrono::{DateTime, Utc};
use sqlx::{AssertSqlSafe, SqliteConnection};
use uuid::fmt::Hyphenated;

use super::Cursor;
use crate::database::identifier;

pub async fn cursor(connection: &mut SqliteConnection) -> Result<Option<Cursor>, sqlx::Error> {
    let row: Option<(DateTime<Utc>, Hyphenated)> =
        sqlx::query_as("SELECT cursor_last_synced_at, cursor_id FROM app_sync_state WHERE id = 1")
            .fetch_optional(connection)
            .await?;
    Ok(row.map(|(last_synced_at, id)| Cursor {
        last_synced_at,
        id: id.into_uuid(),
    }))
}

pub async fn save_cursor(
    connection: &mut SqliteConnection,
    cursor: &Cursor,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO app_sync_state (id, cursor_last_synced_at, cursor_id) VALUES (1, $1, $2)
         ON CONFLICT (id) DO UPDATE SET
           cursor_last_synced_at = excluded.cursor_last_synced_at, cursor_id = excluded.cursor_id",
    )
    .bind(cursor.last_synced_at)
    .bind(cursor.id.hyphenated())
    .execute(connection)
    .await?;
    Ok(())
}

/// Drops a synced table's rows for a resync from nothing, but those an
/// event still pending is about, which the next push answers.
pub async fn delete_not_pending(
    connection: &mut SqliteConnection,
    table: &str,
) -> Result<(), sqlx::Error> {
    let table = identifier(table)?;
    sqlx::query(AssertSqlSafe(format!(
        "DELETE FROM {table} WHERE id NOT IN
           (SELECT row_id FROM app_events WHERE status = 'pending' AND table_name = $1)"
    )))
    .bind(table)
    .execute(connection)
    .await?;
    Ok(())
}

/// Drops the cursor and the backfills to do, for a resync from nothing.
pub async fn delete_cursor_and_backfills(
    connection: &mut SqliteConnection,
) -> Result<(), sqlx::Error> {
    for statement in [
        "DELETE FROM app_sync_state",
        "DELETE FROM app_sync_backfills",
    ] {
        sqlx::query(statement).execute(&mut *connection).await?;
    }
    Ok(())
}
