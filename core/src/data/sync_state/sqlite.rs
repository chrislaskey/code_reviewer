//! The `app_sync_state` table: a device's pull cursor, one row, written by
//! the first pull.

use chrono::{DateTime, Utc};
use sqlx::SqliteConnection;
use uuid::Uuid;
use uuid::fmt::Hyphenated;

use crate::sync::Cursor;

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

/// Drops every synced row, the cursor and the backfills to do, for a
/// resync from nothing. The events stay, and so do the rows they are
/// about while they are pending, and the user's own row, which their
/// posts point at.
pub async fn reset(connection: &mut SqliteConnection, user_id: Uuid) -> Result<(), sqlx::Error> {
    for statement in [
        "DELETE FROM app_posts_communities WHERE id NOT IN
           (SELECT row_id FROM app_events WHERE status = 'pending' AND table_name = 'app_posts_communities')",
        // A file the server refused is still the only copy of its bytes
        "DELETE FROM app_files WHERE id NOT IN
           (SELECT row_id FROM app_events WHERE table_name = 'app_files'
            AND (status = 'pending' OR (status = 'conflict' AND type = 'create')))",
        "DELETE FROM app_posts WHERE id NOT IN
           (SELECT row_id FROM app_events WHERE status = 'pending' AND table_name = 'app_posts')",
        "DELETE FROM app_users_communities WHERE id NOT IN
           (SELECT row_id FROM app_events WHERE status = 'pending' AND table_name = 'app_users_communities')",
        "DELETE FROM app_communities WHERE id NOT IN
           (SELECT row_id FROM app_events WHERE status = 'pending' AND table_name = 'app_communities')",
        "DELETE FROM app_sync_state",
        "DELETE FROM app_sync_backfills",
    ] {
        sqlx::query(statement).execute(&mut *connection).await?;
    }
    sqlx::query("DELETE FROM app_users WHERE id != $1")
        .bind(user_id.hyphenated())
        .execute(connection)
        .await?;
    Ok(())
}
