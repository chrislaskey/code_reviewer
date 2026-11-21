//! The `accounts_credentials` table, the counterpart of `sqlite::credentials`.

use chrono::{DateTime, Utc};
use sqlx::PgConnection;
use uuid::Uuid;

/// The user's hash, in place of any they had.
pub async fn upsert(
    connection: &mut PgConnection,
    user_id: Uuid,
    password_hash: &str,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO accounts_credentials (user_id, password_hash, inserted_at, updated_at)
         VALUES ($1, $2, $3, $3)
         ON CONFLICT (user_id) DO UPDATE SET password_hash = excluded.password_hash, updated_at = excluded.updated_at",
    )
    .bind(user_id)
    .bind(password_hash)
    .bind(now)
    .execute(connection)
    .await?;
    Ok(())
}

pub async fn get_password_hash(
    connection: &mut PgConnection,
    user_id: Uuid,
) -> Result<Option<String>, sqlx::Error> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT password_hash FROM accounts_credentials WHERE user_id = $1")
            .bind(user_id)
            .fetch_optional(connection)
            .await?;
    Ok(row.map(|(hash,)| hash))
}
