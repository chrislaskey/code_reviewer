//! The `accounts_stripe_events` table: the Stripe events already handled,
//! server only.

use chrono::{DateTime, Utc};
use sqlx::{FromRow, SqliteConnection};

use crate::accounts::stripe_events::StripeEvent;

#[derive(FromRow)]
struct StripeEventRow {
    id: String,
    #[sqlx(rename = "type")]
    event_type: String,
    handled_at: DateTime<Utc>,
}

impl From<StripeEventRow> for StripeEvent {
    fn from(row: StripeEventRow) -> StripeEvent {
        StripeEvent {
            id: row.id,
            event_type: row.event_type,
            handled_at: row.handled_at,
        }
    }
}

pub async fn get(
    connection: &mut SqliteConnection,
    id: &str,
) -> Result<Option<StripeEvent>, sqlx::Error> {
    let row: Option<StripeEventRow> =
        sqlx::query_as("SELECT id, type, handled_at FROM accounts_stripe_events WHERE id = $1")
            .bind(id)
            .fetch_optional(connection)
            .await?;
    Ok(row.map(StripeEvent::from))
}

/// How many rows went in: one, or none when the event was handled before.
pub async fn insert_if_missing(
    connection: &mut SqliteConnection,
    id: &str,
    event_type: &str,
    now: DateTime<Utc>,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO accounts_stripe_events (id, type, handled_at) VALUES ($1, $2, $3) ON CONFLICT (id) DO NOTHING",
    )
    .bind(id)
    .bind(event_type)
    .bind(now)
    .execute(connection)
    .await?;
    Ok(result.rows_affected())
}
