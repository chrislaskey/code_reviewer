//! The `app_sync_state` table: a device's pull cursor, one row, written by
//! the first pull. The server never writes it; this is the counterpart
//! of `sqlite::sync_state`.

use sqlx::{AssertSqlSafe, PgConnection};
use uuid::Uuid;

use super::Cursor;
use crate::database::identifier;

pub async fn cursor(connection: &mut PgConnection) -> Result<Option<Cursor>, sqlx::Error> {
    let row: Option<(Uuid, i64)> = sqlx::query_as(
        "SELECT feed_id, global_sync_version FROM app_sync_state
             WHERE id = 1 AND feed_id IS NOT NULL",
    )
    .fetch_optional(connection)
    .await?;
    Ok(row.map(|(feed_id, global_sync_version)| Cursor {
        feed_id,
        global_sync_version,
    }))
}

pub async fn save_cursor(
    connection: &mut PgConnection,
    cursor: &Cursor,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO app_sync_state (id, feed_id, global_sync_version, continuation)
         VALUES (1, $1, $2, NULL)
         ON CONFLICT (id) DO UPDATE SET
           feed_id = excluded.feed_id, global_sync_version = excluded.global_sync_version,
           continuation = NULL",
    )
    .bind(cursor.feed_id)
    .bind(cursor.global_sync_version)
    .execute(connection)
    .await?;
    Ok(())
}

/// The continuation of an unfinished first pull, as JSON.
pub async fn continuation(connection: &mut PgConnection) -> Result<Option<String>, sqlx::Error> {
    let continuation: Option<Option<String>> =
        sqlx::query_scalar("SELECT continuation FROM app_sync_state WHERE id = 1")
            .fetch_optional(connection)
            .await?;
    Ok(continuation.flatten())
}

pub async fn save_continuation(
    connection: &mut PgConnection,
    continuation: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO app_sync_state (id, continuation) VALUES (1, $1)
         ON CONFLICT (id) DO UPDATE SET continuation = excluded.continuation",
    )
    .bind(continuation)
    .execute(connection)
    .await?;
    Ok(())
}

/// Drops a synced table's rows for a resync from nothing, but those an
/// event still pending is about, which the next push answers.
pub async fn delete_not_pending(
    connection: &mut PgConnection,
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
pub async fn delete_cursor_and_backfills(connection: &mut PgConnection) -> Result<(), sqlx::Error> {
    for statement in [
        "DELETE FROM app_sync_state",
        "DELETE FROM app_sync_backfills",
    ] {
        sqlx::query(statement).execute(&mut *connection).await?;
    }
    Ok(())
}

/// Clears the global sync versions of a synced table's rows, kept
/// through a resync, so the new feed's rows replace them.
pub async fn clear_versions(connection: &mut PgConnection, table: &str) -> Result<(), sqlx::Error> {
    let table = identifier(table)?;
    sqlx::query(AssertSqlSafe(format!(
        "UPDATE {table} SET global_sync_version = NULL WHERE global_sync_version IS NOT NULL"
    )))
    .execute(connection)
    .await?;
    Ok(())
}

/// The global sync version of a device's row, none for a row the server
/// has not confirmed, or no such row.
pub async fn row_version(
    connection: &mut PgConnection,
    table: &str,
    id: uuid::Uuid,
) -> Result<Option<i64>, sqlx::Error> {
    let table = identifier(table)?;
    let version: Option<Option<i64>> = sqlx::query_scalar(AssertSqlSafe(format!(
        "SELECT global_sync_version FROM {table} WHERE id = $1"
    )))
    .bind(id)
    .fetch_optional(connection)
    .await?;
    Ok(version.flatten())
}
