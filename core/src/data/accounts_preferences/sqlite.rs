//! The `accounts_preferences` table. Ids are UUIDv7 text and the value
//! JSON text; the row type here turns them into `Preference`'s types.

use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::types::Json;
use sqlx::{FromRow, SqliteConnection};
use uuid::Uuid;
use uuid::fmt::Hyphenated;

use super::Preference;
use crate::data::app_sync_state::Cursor;

#[derive(FromRow)]
struct PreferenceRow {
    id: Hyphenated,
    user_id: Hyphenated,
    key: String,
    value: Json<Value>,
    inserted_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    version: i64,
    last_synced_at: Option<DateTime<Utc>>,
    deleted_at: Option<DateTime<Utc>>,
    deleted_by_id: Option<Hyphenated>,
}

impl From<PreferenceRow> for Preference {
    fn from(row: PreferenceRow) -> Preference {
        Preference {
            id: row.id.into_uuid(),
            user_id: row.user_id.into_uuid(),
            key: row.key,
            value: row.value.0,
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
            "SELECT id, user_id, key, value, inserted_at, updated_at, version, last_synced_at, deleted_at, deleted_by_id FROM accounts_preferences ",
            $($rest),+
        )
    };
}

/// The row by its id, deleted or not.
pub async fn get(
    connection: &mut SqliteConnection,
    id: Uuid,
) -> Result<Option<Preference>, sqlx::Error> {
    let row: Option<PreferenceRow> = sqlx::query_as(select!("WHERE id = $1"))
        .bind(id.hyphenated())
        .fetch_optional(connection)
        .await?;
    Ok(row.map(Preference::from))
}

/// The user's row for the key, deleted or not.
pub async fn get_by_user_and_key(
    connection: &mut SqliteConnection,
    user_id: Uuid,
    key: &str,
) -> Result<Option<Preference>, sqlx::Error> {
    let row: Option<PreferenceRow> = sqlx::query_as(select!("WHERE user_id = $1 AND key = $2"))
        .bind(user_id.hyphenated())
        .bind(key)
        .fetch_optional(connection)
        .await?;
    Ok(row.map(Preference::from))
}

/// The user's live rows, by key.
pub async fn list_by_user(
    connection: &mut SqliteConnection,
    user_id: Uuid,
) -> Result<Vec<Preference>, sqlx::Error> {
    let rows: Vec<PreferenceRow> = sqlx::query_as(select!(
        "WHERE user_id = $1 AND deleted_at IS NULL ORDER BY key"
    ))
    .bind(user_id.hyphenated())
    .fetch_all(connection)
    .await?;
    Ok(rows.into_iter().map(Preference::from).collect())
}

/// The user's rows the server wrote after the cursor, in cursor order.
pub async fn list_synced_after(
    connection: &mut SqliteConnection,
    user_id: Uuid,
    after: &Cursor,
    limit: usize,
) -> Result<Vec<Preference>, sqlx::Error> {
    let rows: Vec<PreferenceRow> = sqlx::query_as(select!(
        "WHERE last_synced_at IS NOT NULL AND (last_synced_at, id) > ($2, $3) AND user_id = $1
         ORDER BY last_synced_at, id LIMIT $4"
    ))
    .bind(user_id.hyphenated())
    .bind(after.last_synced_at)
    .bind(after.id.hyphenated())
    .bind(i64::try_from(limit).unwrap_or(i64::MAX))
    .fetch_all(connection)
    .await?;
    Ok(rows.into_iter().map(Preference::from).collect())
}

pub async fn insert(
    connection: &mut SqliteConnection,
    preference: &Preference,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO accounts_preferences (id, user_id, key, value, inserted_at, updated_at, version, last_synced_at, deleted_at, deleted_by_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
    )
    .bind(preference.id.hyphenated())
    .bind(preference.user_id.hyphenated())
    .bind(&preference.key)
    .bind(Json(&preference.value))
    .bind(preference.inserted_at)
    .bind(preference.updated_at)
    .bind(preference.version)
    .bind(preference.last_synced_at)
    .bind(preference.deleted_at)
    .bind(preference.deleted_by_id.map(Uuid::hyphenated))
    .execute(connection)
    .await?;
    Ok(())
}

/// Writes the value an event may change, with the sync columns.
pub async fn update(
    connection: &mut SqliteConnection,
    preference: &Preference,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE accounts_preferences SET value = $2, updated_at = $3, version = $4, last_synced_at = $5
         WHERE id = $1",
    )
    .bind(preference.id.hyphenated())
    .bind(Json(&preference.value))
    .bind(preference.updated_at)
    .bind(preference.version)
    .bind(preference.last_synced_at)
    .execute(connection)
    .await?;
    Ok(())
}

pub async fn upsert(
    connection: &mut SqliteConnection,
    preference: &Preference,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO accounts_preferences (id, user_id, key, value, inserted_at, updated_at, version, last_synced_at, deleted_at, deleted_by_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
         ON CONFLICT (id) DO UPDATE SET
           user_id = excluded.user_id, key = excluded.key, value = excluded.value,
           inserted_at = excluded.inserted_at, updated_at = excluded.updated_at,
           version = excluded.version, last_synced_at = excluded.last_synced_at,
           deleted_at = excluded.deleted_at, deleted_by_id = excluded.deleted_by_id",
    )
    .bind(preference.id.hyphenated())
    .bind(preference.user_id.hyphenated())
    .bind(&preference.key)
    .bind(Json(&preference.value))
    .bind(preference.inserted_at)
    .bind(preference.updated_at)
    .bind(preference.version)
    .bind(preference.last_synced_at)
    .bind(preference.deleted_at)
    .bind(preference.deleted_by_id.map(Uuid::hyphenated))
    .execute(connection)
    .await?;
    Ok(())
}

pub async fn delete(connection: &mut SqliteConnection, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM accounts_preferences WHERE id = $1")
        .bind(id.hyphenated())
        .execute(connection)
        .await?;
    Ok(())
}

pub async fn delete_of_user(
    connection: &mut SqliteConnection,
    user_id: Uuid,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query("DELETE FROM accounts_preferences WHERE user_id = $1")
        .bind(user_id.hyphenated())
        .execute(connection)
        .await?;
    Ok(result.rows_affected())
}

pub async fn delete_without_account(connection: &mut SqliteConnection) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "DELETE FROM accounts_preferences WHERE user_id NOT IN (SELECT id FROM accounts_users)",
    )
    .execute(connection)
    .await?;
    Ok(result.rows_affected())
}
