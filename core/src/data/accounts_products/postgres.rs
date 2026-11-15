//! The `accounts_products` table, the counterpart of `sqlite::accounts_products`.

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgConnection};
use uuid::Uuid;

use crate::accounts::products::Product;

const COLUMNS: &str = "id, stripe_id, name, description, active, inserted_at, updated_at";

#[derive(FromRow)]
struct ProductRow {
    id: Uuid,
    stripe_id: String,
    name: String,
    description: Option<String>,
    active: bool,
    inserted_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<ProductRow> for Product {
    fn from(row: ProductRow) -> Product {
        Product {
            id: row.id,
            stripe_id: row.stripe_id,
            name: row.name,
            description: row.description,
            active: row.active,
            inserted_at: row.inserted_at,
            updated_at: row.updated_at,
        }
    }
}

pub async fn get(connection: &mut PgConnection, id: Uuid) -> Result<Option<Product>, sqlx::Error> {
    let row: Option<ProductRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM accounts_products WHERE id = $1"
    )))
    .bind(id)
    .fetch_optional(connection)
    .await?;
    Ok(row.map(Product::from))
}

pub async fn get_by_stripe_id(
    connection: &mut PgConnection,
    stripe_id: &str,
) -> Result<Option<Product>, sqlx::Error> {
    let row: Option<ProductRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM accounts_products WHERE stripe_id = $1"
    )))
    .bind(stripe_id)
    .fetch_optional(connection)
    .await?;
    Ok(row.map(Product::from))
}

/// Every product, active or not.
pub async fn list(connection: &mut PgConnection) -> Result<Vec<Product>, sqlx::Error> {
    let rows: Vec<ProductRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM accounts_products ORDER BY name, id"
    )))
    .fetch_all(connection)
    .await?;
    Ok(rows.into_iter().map(Product::from).collect())
}

/// Inserts the product, or updates the one with its Stripe id, keeping
/// that one's id and `inserted_at`.
pub async fn upsert(connection: &mut PgConnection, product: &Product) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO accounts_products (id, stripe_id, name, description, active, inserted_at, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) \
         ON CONFLICT (stripe_id) DO UPDATE SET name = excluded.name, description = excluded.description, \
         active = excluded.active, updated_at = excluded.updated_at",
    )
    .bind(product.id)
    .bind(&product.stripe_id)
    .bind(&product.name)
    .bind(&product.description)
    .bind(product.active)
    .bind(product.inserted_at)
    .bind(product.updated_at)
    .execute(connection)
    .await?;
    Ok(())
}

pub async fn deactivate_all(
    connection: &mut PgConnection,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE accounts_products SET active = FALSE, updated_at = $1 WHERE active")
        .bind(now)
        .execute(connection)
        .await?;
    Ok(())
}

pub async fn deactivate_by_stripe_id(
    connection: &mut PgConnection,
    stripe_id: &str,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE accounts_products SET active = FALSE, updated_at = $2 WHERE stripe_id = $1",
    )
    .bind(stripe_id)
    .bind(now)
    .execute(connection)
    .await?;
    Ok(())
}
