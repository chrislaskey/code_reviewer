//! The `accounts_checkouts` table: checkouts, each with its Stripe Checkout Session, server only.

use chrono::{DateTime, Utc};
use sqlx::{FromRow, SqliteConnection};
use uuid::Uuid;
use uuid::fmt::Hyphenated;

use crate::accounts::checkouts::{Checkout, CheckoutStatus};

const COLUMNS: &str = "id, user_id, price_id, stripe_session_id, status, inserted_at, updated_at";

#[derive(FromRow)]
struct CheckoutRow {
    id: Hyphenated,
    user_id: Hyphenated,
    price_id: Hyphenated,
    stripe_session_id: String,
    status: String,
    inserted_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl TryFrom<CheckoutRow> for Checkout {
    type Error = sqlx::Error;

    fn try_from(row: CheckoutRow) -> Result<Checkout, sqlx::Error> {
        let status = CheckoutStatus::from_name(&row.status).ok_or_else(|| {
            sqlx::Error::Decode(format!("no checkout status {}", row.status).into())
        })?;
        Ok(Checkout {
            id: row.id.into_uuid(),
            user_id: row.user_id.into_uuid(),
            price_id: row.price_id.into_uuid(),
            stripe_session_id: row.stripe_session_id,
            status,
            inserted_at: row.inserted_at,
            updated_at: row.updated_at,
        })
    }
}

pub async fn get(
    connection: &mut SqliteConnection,
    id: Uuid,
) -> Result<Option<Checkout>, sqlx::Error> {
    let row: Option<CheckoutRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM accounts_checkouts WHERE id = $1"
    )))
    .bind(id.hyphenated())
    .fetch_optional(connection)
    .await?;
    row.map(Checkout::try_from).transpose()
}

pub async fn get_by_stripe_session_id(
    connection: &mut SqliteConnection,
    stripe_session_id: &str,
) -> Result<Option<Checkout>, sqlx::Error> {
    let row: Option<CheckoutRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM accounts_checkouts WHERE stripe_session_id = $1"
    )))
    .bind(stripe_session_id)
    .fetch_optional(connection)
    .await?;
    row.map(Checkout::try_from).transpose()
}

pub async fn insert(
    connection: &mut SqliteConnection,
    checkout: &Checkout,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO accounts_checkouts (id, user_id, price_id, stripe_session_id, status, inserted_at, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(checkout.id.hyphenated())
    .bind(checkout.user_id.hyphenated())
    .bind(checkout.price_id.hyphenated())
    .bind(&checkout.stripe_session_id)
    .bind(checkout.status.as_str())
    .bind(checkout.inserted_at)
    .bind(checkout.updated_at)
    .execute(connection)
    .await?;
    Ok(())
}

pub async fn update_status(
    connection: &mut SqliteConnection,
    id: Uuid,
    status: CheckoutStatus,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE accounts_checkouts SET status = $2, updated_at = $3 WHERE id = $1")
        .bind(id.hyphenated())
        .bind(status.as_str())
        .bind(now)
        .execute(connection)
        .await?;
    Ok(())
}

/// The Stripe sessions of the checkouts Stripe may still change.
pub async fn list_pending_stripe_session_ids(
    connection: &mut SqliteConnection,
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT stripe_session_id FROM accounts_checkouts WHERE status IN ('open', 'processing') ORDER BY inserted_at",
    )
    .fetch_all(connection)
    .await
}
