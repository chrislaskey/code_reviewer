//! The `app_events` table. Ids are UUIDv7 text and the JSON fields text,
//! so the file reads well in the sqlite3 shell.

use chrono::{DateTime, Utc};
use sqlx::types::Json;
use sqlx::{FromRow, SqliteConnection};
use uuid::Uuid;
use uuid::fmt::Hyphenated;

use crate::events::{Conflicts, Event, EventKind, EventStatus, Fields, StoredEvent};

#[derive(FromRow)]
struct EventRow {
    id: Hyphenated,
    user_id: Hyphenated,
    table_name: String,
    row_id: Hyphenated,
    #[sqlx(rename = "type")]
    kind: String,
    base_version: i64,
    base: Json<Fields>,
    changes: Json<Fields>,
    made_at: DateTime<Utc>,
    status: String,
    conflicts: Option<Json<Conflicts>>,
    version: Option<i64>,
    last_error: Option<String>,
    attempts: i64,
}

impl TryFrom<EventRow> for StoredEvent {
    type Error = sqlx::Error;

    fn try_from(row: EventRow) -> Result<StoredEvent, sqlx::Error> {
        let kind = EventKind::parse(&row.kind).ok_or_else(|| {
            sqlx::Error::Decode(format!("unknown event type '{}'", row.kind).into())
        })?;
        let status = EventStatus::parse(&row.status).ok_or_else(|| {
            sqlx::Error::Decode(format!("unknown event status '{}'", row.status).into())
        })?;
        Ok(StoredEvent {
            event: Event {
                id: row.id.into_uuid(),
                table: row.table_name,
                row_id: row.row_id.into_uuid(),
                kind,
                base_version: row.base_version,
                base: row.base.0,
                changes: row.changes.0,
                made_at: row.made_at,
            },
            user_id: row.user_id.into_uuid(),
            status,
            conflicts: row.conflicts.map(|json| json.0).unwrap_or_default(),
            version: row.version,
            last_error: row.last_error,
            attempts: row.attempts,
        })
    }
}

macro_rules! select {
    ($rest:literal) => {
        concat!(
            "SELECT id, user_id, table_name, row_id, type, base_version, base, changes, made_at, status, conflicts, version, last_error, attempts FROM app_events ",
            $rest
        )
    };
}

pub async fn insert(
    connection: &mut SqliteConnection,
    stored: &StoredEvent,
) -> Result<(), sqlx::Error> {
    let event = &stored.event;
    sqlx::query(
        "INSERT INTO app_events (id, user_id, table_name, row_id, type, base_version, base, changes, made_at, status, conflicts, version, last_error)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)",
    )
    .bind(event.id.hyphenated())
    .bind(stored.user_id.hyphenated())
    .bind(&event.table)
    .bind(event.row_id.hyphenated())
    .bind(event.kind.as_str())
    .bind(event.base_version)
    .bind(Json(&event.base))
    .bind(Json(&event.changes))
    .bind(event.made_at)
    .bind(stored.status.as_str())
    .bind(Json(&stored.conflicts))
    .bind(stored.version)
    .bind(&stored.last_error)
    .execute(connection)
    .await?;
    Ok(())
}

pub async fn get(
    connection: &mut SqliteConnection,
    id: Uuid,
) -> Result<Option<StoredEvent>, sqlx::Error> {
    let row: Option<EventRow> = sqlx::query_as(select!("WHERE id = $1"))
        .bind(id.hyphenated())
        .fetch_optional(connection)
        .await?;
    row.map(StoredEvent::try_from).transpose()
}

/// The events with the status, in the order they were made.
pub async fn list_by_status(
    connection: &mut SqliteConnection,
    status: EventStatus,
) -> Result<Vec<StoredEvent>, sqlx::Error> {
    let rows: Vec<EventRow> = sqlx::query_as(select!("WHERE status = $1 ORDER BY made_at, id"))
        .bind(status.as_str())
        .fetch_all(connection)
        .await?;
    rows.into_iter().map(StoredEvent::try_from).collect()
}

/// The events of the table's rows, in the order they were made: on a
/// device, the ones waiting and the ones the person has not resolved.
pub async fn list_by_table(
    connection: &mut SqliteConnection,
    table: &str,
) -> Result<Vec<StoredEvent>, sqlx::Error> {
    let rows: Vec<EventRow> = sqlx::query_as(select!("WHERE table_name = $1 ORDER BY made_at, id"))
        .bind(table)
        .fetch_all(connection)
        .await?;
    rows.into_iter().map(StoredEvent::try_from).collect()
}

pub async fn count_by_status(
    connection: &mut SqliteConnection,
    status: EventStatus,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT COUNT(*) FROM app_events WHERE status = $1")
        .bind(status.as_str())
        .fetch_one(connection)
        .await
}

pub async fn has_pending(
    connection: &mut SqliteConnection,
    table: &str,
    row_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM app_events WHERE table_name = $1 AND row_id = $2 AND status = 'pending'",
    )
    .bind(table)
    .bind(row_id.hyphenated())
    .fetch_one(connection)
    .await?;
    Ok(count > 0)
}

pub async fn delete(connection: &mut SqliteConnection, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM app_events WHERE id = $1")
        .bind(id.hyphenated())
        .execute(connection)
        .await?;
    Ok(())
}

pub async fn delete_pending_of_row(
    connection: &mut SqliteConnection,
    table: &str,
    row_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "DELETE FROM app_events WHERE table_name = $1 AND row_id = $2 AND status = 'pending'",
    )
    .bind(table)
    .bind(row_id.hyphenated())
    .execute(connection)
    .await?;
    Ok(())
}

/// Removes every event of the row, whatever its status: for a row that
/// never reached the server and is going from the device.
pub async fn delete_of_row(
    connection: &mut SqliteConnection,
    table: &str,
    row_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM app_events WHERE table_name = $1 AND row_id = $2")
        .bind(table)
        .bind(row_id.hyphenated())
        .execute(connection)
        .await?;
    Ok(())
}

pub async fn mark_conflict(
    connection: &mut SqliteConnection,
    id: Uuid,
    conflicts: &Conflicts,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE app_events SET status = 'conflict', conflicts = $2 WHERE id = $1")
        .bind(id.hyphenated())
        .bind(Json(conflicts))
        .execute(connection)
        .await?;
    Ok(())
}

/// Counts one more try the server or the bucket said no to, with why;
/// answers how many that makes.
pub async fn add_attempt(
    connection: &mut SqliteConnection,
    id: Uuid,
    reason: &str,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "UPDATE app_events SET attempts = attempts + 1, last_error = $2 WHERE id = $1 RETURNING attempts",
    )
    .bind(id.hyphenated())
    .bind(reason)
    .fetch_one(connection)
    .await
}

/// Stops trying the event: kept as a conflict with why, for the person.
pub async fn mark_refused(
    connection: &mut SqliteConnection,
    id: Uuid,
    conflicts: &Conflicts,
    reason: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE app_events SET status = 'conflict', conflicts = $2, last_error = $3 WHERE id = $1",
    )
    .bind(id.hyphenated())
    .bind(Json(conflicts))
    .bind(reason)
    .execute(connection)
    .await?;
    Ok(())
}

/// Puts the event back in the queue as if it had never been tried.
pub async fn mark_pending(connection: &mut SqliteConnection, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE app_events SET status = 'pending', conflicts = NULL, last_error = NULL, attempts = 0 WHERE id = $1",
    )
    .bind(id.hyphenated())
    .execute(connection)
    .await?;
    Ok(())
}

pub async fn mark_error(
    connection: &mut SqliteConnection,
    id: Uuid,
    reason: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE app_events SET last_error = $2 WHERE id = $1")
        .bind(id.hyphenated())
        .bind(reason)
        .execute(connection)
        .await?;
    Ok(())
}

/// Removes the events of rows that are gone, after a purge.
pub async fn delete_of_missing_rows(connection: &mut SqliteConnection) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "DELETE FROM app_events WHERE
           (table_name = 'app_posts' AND row_id NOT IN (SELECT id FROM app_posts))
           OR (table_name = 'app_files' AND row_id NOT IN (SELECT id FROM app_files))
           OR (table_name = 'app_posts_communities' AND row_id NOT IN (SELECT id FROM app_posts_communities))
           OR (table_name = 'app_users_communities' AND row_id NOT IN (SELECT id FROM app_users_communities))
           OR (table_name = 'app_communities' AND row_id NOT IN (SELECT id FROM app_communities))
           OR (table_name = 'app_users' AND row_id NOT IN (SELECT id FROM app_users))",
    )
    .execute(connection)
    .await?;
    Ok(result.rows_affected())
}
