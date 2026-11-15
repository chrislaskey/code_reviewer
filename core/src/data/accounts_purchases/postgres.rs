//! The `accounts_purchases` table, the counterpart of `sqlite::accounts_purchases`.

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgConnection};
use uuid::Uuid;

use crate::accounts::purchases::{Purchase, PurchaseShown, Store};

const SELECT: &str = "SELECT accounts_purchases.id, user_id, product_id, price_id, checkout_id, store, store_id, \
     amount, currency, refunded_at, accounts_purchases.inserted_at, accounts_purchases.updated_at, \
     accounts_products.name AS product_name \
     FROM accounts_purchases JOIN accounts_products ON accounts_products.id = accounts_purchases.product_id";

#[derive(FromRow)]
struct PurchaseRow {
    id: Uuid,
    user_id: Uuid,
    product_id: Uuid,
    price_id: Uuid,
    checkout_id: Option<Uuid>,
    store: String,
    store_id: String,
    amount: i64,
    currency: String,
    refunded_at: Option<DateTime<Utc>>,
    inserted_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    product_name: String,
}

impl TryFrom<PurchaseRow> for PurchaseShown {
    type Error = sqlx::Error;

    fn try_from(row: PurchaseRow) -> Result<PurchaseShown, sqlx::Error> {
        let store = Store::from_name(&row.store)
            .ok_or_else(|| sqlx::Error::Decode(format!("no store {}", row.store).into()))?;
        Ok(PurchaseShown {
            purchase: Purchase {
                id: row.id,
                user_id: row.user_id,
                product_id: row.product_id,
                price_id: row.price_id,
                checkout_id: row.checkout_id,
                store,
                store_id: row.store_id,
                amount: row.amount,
                currency: row.currency,
                refunded_at: row.refunded_at,
                inserted_at: row.inserted_at,
                updated_at: row.updated_at,
            },
            product_name: row.product_name,
        })
    }
}

pub async fn get(
    connection: &mut PgConnection,
    id: Uuid,
) -> Result<Option<PurchaseShown>, sqlx::Error> {
    let row: Option<PurchaseRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "{SELECT} WHERE accounts_purchases.id = $1"
    )))
    .bind(id)
    .fetch_optional(connection)
    .await?;
    row.map(PurchaseShown::try_from).transpose()
}

/// The purchase a checkout paid for, if it has one.
pub async fn get_by_checkout_id(
    connection: &mut PgConnection,
    checkout_id: Uuid,
) -> Result<Option<Uuid>, sqlx::Error> {
    let id: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM accounts_purchases WHERE checkout_id = $1")
            .bind(checkout_id)
            .fetch_optional(connection)
            .await?;
    Ok(id)
}

/// The user's purchases, newest first.
pub async fn list_by_user(
    connection: &mut PgConnection,
    user_id: Uuid,
) -> Result<Vec<PurchaseShown>, sqlx::Error> {
    let rows: Vec<PurchaseRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "{SELECT} WHERE user_id = $1 ORDER BY accounts_purchases.inserted_at DESC, accounts_purchases.id DESC"
    )))
    .bind(user_id)
    .fetch_all(connection)
    .await?;
    rows.into_iter().map(PurchaseShown::try_from).collect()
}

/// Inserts the purchase unless one has its store's id.
pub async fn insert_if_missing(
    connection: &mut PgConnection,
    purchase: &Purchase,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO accounts_purchases \
         (id, user_id, product_id, price_id, checkout_id, store, store_id, amount, currency, refunded_at, \
         inserted_at, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12) \
         ON CONFLICT (store, store_id) DO NOTHING",
    )
    .bind(purchase.id)
    .bind(purchase.user_id)
    .bind(purchase.product_id)
    .bind(purchase.price_id)
    .bind(purchase.checkout_id)
    .bind(purchase.store.as_str())
    .bind(&purchase.store_id)
    .bind(purchase.amount)
    .bind(&purchase.currency)
    .bind(purchase.refunded_at)
    .bind(purchase.inserted_at)
    .bind(purchase.updated_at)
    .execute(connection)
    .await?;
    Ok(())
}

/// Whether the user bought the product and it was not refunded.
pub async fn owns(
    connection: &mut PgConnection,
    user_id: Uuid,
    product_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM accounts_purchases WHERE user_id = $1 AND product_id = $2 AND refunded_at IS NULL",
    )
    .bind(user_id)
    .bind(product_id)
    .fetch_one(connection)
    .await?;
    Ok(count > 0)
}

/// Marks refunded the purchase with the store's id; a refund noted
/// before keeps its time.
pub async fn refund(
    connection: &mut PgConnection,
    store: Store,
    store_id: &str,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE accounts_purchases SET refunded_at = $3, updated_at = $3 \
         WHERE store = $1 AND store_id = $2 AND refunded_at IS NULL",
    )
    .bind(store.as_str())
    .bind(store_id)
    .bind(now)
    .execute(connection)
    .await?;
    Ok(())
}

/// The id of the row with the store's id, and its user's, if there is one.
pub async fn get_ids_by_store_id(
    connection: &mut PgConnection,
    store: Store,
    store_id: &str,
) -> Result<Option<(Uuid, Uuid)>, sqlx::Error> {
    sqlx::query_as("SELECT id, user_id FROM accounts_purchases WHERE store = $1 AND store_id = $2")
        .bind(store.as_str())
        .bind(store_id)
        .fetch_optional(connection)
        .await
}
