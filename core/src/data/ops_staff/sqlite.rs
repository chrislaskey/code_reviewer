//! The `ops_staff` table: the accounts that may use the ops app, server
//! only.

use chrono::{DateTime, Utc};
use sqlx::{FromRow, SqliteConnection};
use uuid::Uuid;
use uuid::fmt::Hyphenated;

use crate::ops::staff::Staff;

#[derive(FromRow)]
struct StaffRow {
    user_id: Hyphenated,
    inserted_at: DateTime<Utc>,
}

impl From<StaffRow> for Staff {
    fn from(row: StaffRow) -> Staff {
        Staff {
            user_id: row.user_id.into_uuid(),
            inserted_at: row.inserted_at,
        }
    }
}

pub async fn get(
    connection: &mut SqliteConnection,
    user_id: Uuid,
) -> Result<Option<Staff>, sqlx::Error> {
    let row: Option<StaffRow> =
        sqlx::query_as("SELECT user_id, inserted_at FROM ops_staff WHERE user_id = $1")
            .bind(user_id.hyphenated())
            .fetch_optional(connection)
            .await?;
    Ok(row.map(Staff::from))
}

pub async fn insert_if_missing(
    connection: &mut SqliteConnection,
    user_id: Uuid,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO ops_staff (user_id, inserted_at) VALUES ($1, $2) ON CONFLICT (user_id) DO NOTHING",
    )
    .bind(user_id.hyphenated())
    .bind(now)
    .execute(connection)
    .await?;
    Ok(())
}

/// How many rows went: one, or none when the user was not staff.
pub async fn delete(connection: &mut SqliteConnection, user_id: Uuid) -> Result<u64, sqlx::Error> {
    let result = sqlx::query("DELETE FROM ops_staff WHERE user_id = $1")
        .bind(user_id.hyphenated())
        .execute(connection)
        .await?;
    Ok(result.rows_affected())
}
