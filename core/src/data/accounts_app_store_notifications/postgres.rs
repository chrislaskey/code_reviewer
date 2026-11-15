//! The `accounts_app_store_notifications` table, the counterpart of
//! `sqlite::accounts_app_store_notifications`.

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgConnection};

use crate::accounts::app_store_notifications::AppStoreNotification;

#[derive(FromRow)]
struct AppStoreNotificationRow {
    id: String,
    #[sqlx(rename = "type")]
    notification_type: String,
    handled_at: DateTime<Utc>,
}

impl From<AppStoreNotificationRow> for AppStoreNotification {
    fn from(row: AppStoreNotificationRow) -> AppStoreNotification {
        AppStoreNotification {
            id: row.id,
            notification_type: row.notification_type,
            handled_at: row.handled_at,
        }
    }
}

pub async fn get(
    connection: &mut PgConnection,
    id: &str,
) -> Result<Option<AppStoreNotification>, sqlx::Error> {
    let row: Option<AppStoreNotificationRow> = sqlx::query_as(
        "SELECT id, type, handled_at FROM accounts_app_store_notifications WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(connection)
    .await?;
    Ok(row.map(AppStoreNotification::from))
}

/// How many rows went in: one, or none when the notification was handled before.
pub async fn insert_if_missing(
    connection: &mut PgConnection,
    id: &str,
    notification_type: &str,
    now: DateTime<Utc>,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO accounts_app_store_notifications (id, type, handled_at) VALUES ($1, $2, $3) ON CONFLICT (id) DO NOTHING",
    )
    .bind(id)
    .bind(notification_type)
    .bind(now)
    .execute(connection)
    .await?;
    Ok(result.rows_affected())
}
