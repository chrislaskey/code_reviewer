//! The `app_test_notes` table on SQLite, ids as UUIDv7 text. What a user
//! sees is one predicate, `visible_to!`, for every read.

use chrono::{DateTime, Utc};
use sqlx::{FromRow, SqliteConnection};
use uuid::Uuid;
use uuid::fmt::Hyphenated;

use super::Note;
use crate::data::app_sync_state::Cursor;

#[derive(FromRow)]
struct NoteRow {
    id: Hyphenated,
    user_id: Hyphenated,
    community_id: Option<Hyphenated>,
    body: String,
    inserted_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    version: i64,
    last_synced_at: Option<DateTime<Utc>>,
    deleted_at: Option<DateTime<Utc>>,
    deleted_by_id: Option<Hyphenated>,
}

impl From<NoteRow> for Note {
    fn from(row: NoteRow) -> Note {
        Note {
            id: row.id.into_uuid(),
            user_id: row.user_id.into_uuid(),
            community_id: row.community_id.map(Hyphenated::into_uuid),
            body: row.body,
            inserted_at: row.inserted_at,
            updated_at: row.updated_at,
            version: row.version,
            last_synced_at: row.last_synced_at,
            deleted_at: row.deleted_at,
            deleted_by_id: row.deleted_by_id.map(Hyphenated::into_uuid),
        }
    }
}

macro_rules! select {
    ($($rest:expr),+ $(,)?) => {
        concat!(
            "SELECT id, user_id, community_id, body, inserted_at, updated_at, version, last_synced_at, deleted_at, deleted_by_id FROM app_test_notes ",
            $($rest),+
        )
    };
}

/// The notes the user, `$1`, sees: their own, and every note in a
/// community they are a live member of.
macro_rules! visible_to {
    () => {
        "(user_id = $1 OR community_id IN (SELECT community_id FROM app_users_communities WHERE user_id = $1 AND deleted_at IS NULL))"
    };
}

pub async fn list_visible(
    connection: &mut SqliteConnection,
    user_id: Uuid,
) -> Result<Vec<Note>, sqlx::Error> {
    let rows: Vec<NoteRow> = sqlx::query_as(select!(
        "WHERE deleted_at IS NULL AND ",
        visible_to!(),
        " ORDER BY inserted_at DESC, id DESC"
    ))
    .bind(user_id.hyphenated())
    .fetch_all(connection)
    .await?;
    Ok(rows.into_iter().map(Note::from).collect())
}

pub async fn list_live_of_user(
    connection: &mut SqliteConnection,
    user_id: Uuid,
) -> Result<Vec<Note>, sqlx::Error> {
    let rows: Vec<NoteRow> = sqlx::query_as(select!(
        "WHERE deleted_at IS NULL AND user_id = $1 ORDER BY inserted_at, id"
    ))
    .bind(user_id.hyphenated())
    .fetch_all(connection)
    .await?;
    Ok(rows.into_iter().map(Note::from).collect())
}

pub async fn list_live_in_community(
    connection: &mut SqliteConnection,
    community_id: Uuid,
) -> Result<Vec<Note>, sqlx::Error> {
    let rows: Vec<NoteRow> = sqlx::query_as(select!(
        "WHERE deleted_at IS NULL AND community_id = $1 ORDER BY inserted_at, id"
    ))
    .bind(community_id.hyphenated())
    .fetch_all(connection)
    .await?;
    Ok(rows.into_iter().map(Note::from).collect())
}

pub async fn list_live_of_user_in_community(
    connection: &mut SqliteConnection,
    user_id: Uuid,
    community_id: Uuid,
) -> Result<Vec<Note>, sqlx::Error> {
    let rows: Vec<NoteRow> = sqlx::query_as(select!(
        "WHERE deleted_at IS NULL AND user_id = $1 AND community_id = $2 ORDER BY inserted_at, id"
    ))
    .bind(user_id.hyphenated())
    .bind(community_id.hyphenated())
    .fetch_all(connection)
    .await?;
    Ok(rows.into_iter().map(Note::from).collect())
}

pub async fn list_of_user_in_community_deleted_at(
    connection: &mut SqliteConnection,
    user_id: Uuid,
    community_id: Uuid,
    deleted_at: DateTime<Utc>,
) -> Result<Vec<Note>, sqlx::Error> {
    let rows: Vec<NoteRow> = sqlx::query_as(select!(
        "WHERE deleted_at = $3 AND user_id = $1 AND community_id = $2 ORDER BY inserted_at, id"
    ))
    .bind(user_id.hyphenated())
    .bind(community_id.hyphenated())
    .bind(deleted_at)
    .fetch_all(connection)
    .await?;
    Ok(rows.into_iter().map(Note::from).collect())
}

pub async fn list_live_in_community_after(
    connection: &mut SqliteConnection,
    community_id: Uuid,
    after: &Cursor,
    limit: usize,
) -> Result<Vec<Note>, sqlx::Error> {
    let rows: Vec<NoteRow> = sqlx::query_as(select!(
        "WHERE deleted_at IS NULL AND community_id = $1 AND last_synced_at IS NOT NULL
           AND (last_synced_at, id) > ($2, $3) ORDER BY last_synced_at, id LIMIT $4"
    ))
    .bind(community_id.hyphenated())
    .bind(after.last_synced_at)
    .bind(after.id.hyphenated())
    .bind(i64::try_from(limit).unwrap_or(i64::MAX))
    .fetch_all(connection)
    .await?;
    Ok(rows.into_iter().map(Note::from).collect())
}

pub async fn list_synced_after(
    connection: &mut SqliteConnection,
    user_id: Uuid,
    after: &Cursor,
    limit: usize,
) -> Result<Vec<Note>, sqlx::Error> {
    let rows: Vec<NoteRow> = sqlx::query_as(select!(
        "WHERE last_synced_at IS NOT NULL AND (last_synced_at, id) > ($2, $3) AND ",
        visible_to!(),
        " ORDER BY last_synced_at, id LIMIT $4"
    ))
    .bind(user_id.hyphenated())
    .bind(after.last_synced_at)
    .bind(after.id.hyphenated())
    .bind(i64::try_from(limit).unwrap_or(i64::MAX))
    .fetch_all(connection)
    .await?;
    Ok(rows.into_iter().map(Note::from).collect())
}

pub async fn get(connection: &mut SqliteConnection, id: Uuid) -> Result<Option<Note>, sqlx::Error> {
    let row: Option<NoteRow> = sqlx::query_as(select!("WHERE id = $1"))
        .bind(id.hyphenated())
        .fetch_optional(connection)
        .await?;
    Ok(row.map(Note::from))
}

pub async fn get_visible(
    connection: &mut SqliteConnection,
    user_id: Uuid,
    id: Uuid,
) -> Result<Option<Note>, sqlx::Error> {
    let row: Option<NoteRow> = sqlx::query_as(select!("WHERE id = $2 AND ", visible_to!()))
        .bind(user_id.hyphenated())
        .bind(id.hyphenated())
        .fetch_optional(connection)
        .await?;
    Ok(row.map(Note::from))
}

pub async fn insert(connection: &mut SqliteConnection, note: &Note) -> Result<(), sqlx::Error> {
    upsert(connection, note).await
}

pub async fn update(connection: &mut SqliteConnection, note: &Note) -> Result<(), sqlx::Error> {
    upsert(connection, note).await
}

pub async fn upsert(connection: &mut SqliteConnection, note: &Note) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO app_test_notes (id, user_id, community_id, body, inserted_at, updated_at, version, last_synced_at, deleted_at, deleted_by_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
         ON CONFLICT (id) DO UPDATE SET
           user_id = excluded.user_id, community_id = excluded.community_id, body = excluded.body,
           inserted_at = excluded.inserted_at, updated_at = excluded.updated_at, version = excluded.version,
           last_synced_at = excluded.last_synced_at, deleted_at = excluded.deleted_at,
           deleted_by_id = excluded.deleted_by_id",
    )
    .bind(note.id.hyphenated())
    .bind(note.user_id.hyphenated())
    .bind(note.community_id.map(Uuid::hyphenated))
    .bind(&note.body)
    .bind(note.inserted_at)
    .bind(note.updated_at)
    .bind(note.version)
    .bind(note.last_synced_at)
    .bind(note.deleted_at)
    .bind(note.deleted_by_id.map(Uuid::hyphenated))
    .execute(connection)
    .await?;
    Ok(())
}

pub async fn update_community_of_all(
    connection: &mut SqliteConnection,
    from_community_id: Uuid,
    to_community_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE app_test_notes SET community_id = $2 WHERE community_id = $1")
        .bind(from_community_id.hyphenated())
        .bind(to_community_id.hyphenated())
        .execute(connection)
        .await?;
    Ok(())
}

/// Removes every note the user does not own and no longer sees; how
/// many went.
pub async fn delete_unreachable(
    connection: &mut SqliteConnection,
    user_id: Uuid,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(concat!(
        "DELETE FROM app_test_notes WHERE user_id != $1 AND NOT ",
        visible_to!()
    ))
    .bind(user_id.hyphenated())
    .execute(connection)
    .await?;
    Ok(result.rows_affected())
}

pub async fn delete_row(connection: &mut SqliteConnection, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM app_test_notes WHERE id = $1")
        .bind(id.hyphenated())
        .execute(connection)
        .await?;
    Ok(())
}
