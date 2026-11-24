//! The `device_settings` table: a jsonb value per key, the counterpart
//! of `sqlite::device_settings`.

use serde_json::Value;
use sqlx::PgConnection;
use sqlx::types::Json;

pub async fn get(connection: &mut PgConnection, key: &str) -> Result<Option<Value>, sqlx::Error> {
    let row: Option<(Json<Value>,)> =
        sqlx::query_as("SELECT value FROM device_settings WHERE key = $1")
            .bind(key)
            .fetch_optional(connection)
            .await?;
    Ok(row.map(|(value,)| value.0))
}

pub async fn set(
    connection: &mut PgConnection,
    key: &str,
    value: &Value,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO device_settings (key, value) VALUES ($1, $2)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
    )
    .bind(key)
    .bind(Json(value))
    .execute(connection)
    .await?;
    Ok(())
}
