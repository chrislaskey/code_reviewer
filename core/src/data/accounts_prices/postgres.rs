//! The `accounts_prices` table, the counterpart of `sqlite::accounts_prices`.

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgConnection};

use uuid::Uuid;

use crate::accounts::prices::{Price, PriceInterval};

const COLUMNS: &str = "id, product_id, stripe_id, lookup_key, amount, currency, interval, active, inserted_at, updated_at";

#[derive(FromRow)]
struct PriceRow {
    id: Uuid,
    product_id: Uuid,
    stripe_id: String,
    lookup_key: Option<String>,
    amount: i64,
    currency: String,
    interval: Option<String>,
    active: bool,
    inserted_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<PriceRow> for Price {
    fn from(row: PriceRow) -> Price {
        Price {
            id: row.id,
            product_id: row.product_id,
            stripe_id: row.stripe_id,
            lookup_key: row.lookup_key,
            amount: row.amount,
            currency: row.currency,
            interval: row.interval.as_deref().and_then(PriceInterval::from_name),
            active: row.active,
            inserted_at: row.inserted_at,
            updated_at: row.updated_at,
        }
    }
}

pub async fn get(connection: &mut PgConnection, id: Uuid) -> Result<Option<Price>, sqlx::Error> {
    let row: Option<PriceRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM accounts_prices WHERE id = $1"
    )))
    .bind(id)
    .fetch_optional(connection)
    .await?;
    Ok(row.map(Price::from))
}

pub async fn get_by_stripe_id(
    connection: &mut PgConnection,
    stripe_id: &str,
) -> Result<Option<Price>, sqlx::Error> {
    let row: Option<PriceRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM accounts_prices WHERE stripe_id = $1"
    )))
    .bind(stripe_id)
    .fetch_optional(connection)
    .await?;
    Ok(row.map(Price::from))
}

/// Every price, active or not.
pub async fn list(connection: &mut PgConnection) -> Result<Vec<Price>, sqlx::Error> {
    let rows: Vec<PriceRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM accounts_prices ORDER BY product_id, amount, id"
    )))
    .fetch_all(connection)
    .await?;
    Ok(rows.into_iter().map(Price::from).collect())
}

/// Inserts the price, or updates the one with its Stripe id, keeping
/// that one's id and `inserted_at`.
pub async fn upsert(connection: &mut PgConnection, price: &Price) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO accounts_prices \
         (id, product_id, stripe_id, lookup_key, amount, currency, interval, active, inserted_at, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) \
         ON CONFLICT (stripe_id) DO UPDATE SET product_id = excluded.product_id, \
         lookup_key = excluded.lookup_key, amount = excluded.amount, currency = excluded.currency, \
         interval = excluded.interval, active = excluded.active, updated_at = excluded.updated_at",
    )
    .bind(price.id)
    .bind(price.product_id)
    .bind(&price.stripe_id)
    .bind(&price.lookup_key)
    .bind(price.amount)
    .bind(&price.currency)
    .bind(price.interval.map(PriceInterval::as_str))
    .bind(price.active)
    .bind(price.inserted_at)
    .bind(price.updated_at)
    .execute(connection)
    .await?;
    Ok(())
}

pub async fn deactivate_all(
    connection: &mut PgConnection,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE accounts_prices SET active = FALSE, updated_at = $1 WHERE active")
        .bind(now)
        .execute(connection)
        .await?;
    Ok(())
}
