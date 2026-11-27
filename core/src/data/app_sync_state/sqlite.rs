//! The `app_sync_state` table: a device's pull cursor, one row, written by
//! the first pull.

use sqlx::{AssertSqlSafe, SqliteConnection};
use uuid::fmt::Hyphenated;

use super::Cursor;
use crate::database::identifier;

pub async fn cursor(connection: &mut SqliteConnection) -> Result<Option<Cursor>, sqlx::Error> {
    let row: Option<(Hyphenated, i64)> =
        sqlx::query_as("SELECT feed_id, global_sync_version FROM app_sync_state WHERE id = 1")
            .fetch_optional(connection)
            .await?;
    Ok(row.map(|(feed_id, global_sync_version)| Cursor {
        feed_id: feed_id.into_uuid(),
        global_sync_version,
    }))
}

pub async fn save_cursor(
    connection: &mut SqliteConnection,
    cursor: &Cursor,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO app_sync_state (id, feed_id, global_sync_version) VALUES (1, $1, $2)
         ON CONFLICT (id) DO UPDATE SET
           feed_id = excluded.feed_id, global_sync_version = excluded.global_sync_version",
    )
    .bind(cursor.feed_id.hyphenated())
    .bind(cursor.global_sync_version)
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
    let without_dependents = crate::sync::tables::without_dependents(table)?;
    sqlx::query(AssertSqlSafe(format!(
        "DELETE FROM {table} WHERE id NOT IN
           (SELECT row_id FROM app_events WHERE status = 'pending' AND table_name = $1){without_dependents}"
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
