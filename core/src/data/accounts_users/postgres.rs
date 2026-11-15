//! The `accounts_users` table, the counterpart of `sqlite::accounts_users`.

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgConnection};
use uuid::Uuid;

use crate::accounts::users::AccountUser;

#[derive(FromRow)]
struct AccountUserRow {
    id: Uuid,
    email: String,
    confirmed_at: Option<DateTime<Utc>>,
    trial_expires_at: Option<DateTime<Utc>>,
    inserted_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    deleted_at: Option<DateTime<Utc>>,
}

impl From<AccountUserRow> for AccountUser {
    fn from(row: AccountUserRow) -> AccountUser {
        AccountUser {
            id: row.id,
            email: row.email,
            confirmed_at: row.confirmed_at,
            trial_expires_at: row.trial_expires_at,
            inserted_at: row.inserted_at,
            updated_at: row.updated_at,
            deleted_at: row.deleted_at,
        }
    }
}

macro_rules! select {
    ($rest:expr) => {
        concat!(
            "SELECT id, email, confirmed_at, trial_expires_at, inserted_at, updated_at, deleted_at FROM accounts_users ",
            $rest
        )
    };
}

/// The row by its id, deleted or not.
pub async fn get(
    connection: &mut PgConnection,
    id: Uuid,
) -> Result<Option<AccountUser>, sqlx::Error> {
    let row: Option<AccountUserRow> = sqlx::query_as(select!("WHERE id = $1"))
        .bind(id)
        .fetch_optional(connection)
        .await?;
    Ok(row.map(AccountUser::from))
}

/// The live account with the email.
pub async fn get_by_email(
    connection: &mut PgConnection,
    email: &str,
) -> Result<Option<AccountUser>, sqlx::Error> {
    let row: Option<AccountUserRow> =
        sqlx::query_as(select!("WHERE email = $1 AND deleted_at IS NULL"))
            .bind(email)
            .fetch_optional(connection)
            .await?;
    Ok(row.map(AccountUser::from))
}

pub async fn insert(
    connection: &mut PgConnection,
    account: &AccountUser,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO accounts_users (id, email, confirmed_at, trial_expires_at, inserted_at, updated_at, deleted_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(account.id)
    .bind(&account.email)
    .bind(account.confirmed_at)
    .bind(account.trial_expires_at)
    .bind(account.inserted_at)
    .bind(account.updated_at)
    .bind(account.deleted_at)
    .execute(connection)
    .await?;
    Ok(())
}

/// Marks the account's email as proved.
pub async fn set_confirmed_at(
    connection: &mut PgConnection,
    id: Uuid,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE accounts_users SET confirmed_at = $2, updated_at = $2 WHERE id = $1")
        .bind(id)
        .bind(now)
        .execute(connection)
        .await?;
    Ok(())
}

/// Marks the account deleted, see `accounts::deletions`.
pub async fn set_deleted_at(
    connection: &mut PgConnection,
    id: Uuid,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE accounts_users SET deleted_at = $2, updated_at = $2 WHERE id = $1")
        .bind(id)
        .bind(now)
        .execute(connection)
        .await?;
    Ok(())
}

/// Deletes the accounts that never proved their email and were made
/// before the time, with their credentials and tokens; how many. Their
/// `app_users` rows go first, see `users::delete_of_unconfirmed_accounts_before`.
pub async fn delete_unconfirmed_before(
    connection: &mut PgConnection,
    time: DateTime<Utc>,
) -> Result<u64, sqlx::Error> {
    let result =
        sqlx::query("DELETE FROM accounts_users WHERE confirmed_at IS NULL AND trial_expires_at IS NULL AND inserted_at < $1")
            .bind(time)
            .execute(connection)
            .await?;
    Ok(result.rows_affected())
}

/// Deletes the accounts whose `app_users` row the purge removed, with
/// their credentials and tokens; how many.
pub async fn delete_without_user(connection: &mut PgConnection) -> Result<u64, sqlx::Error> {
    let result =
        sqlx::query("DELETE FROM accounts_users WHERE id NOT IN (SELECT id FROM app_users)")
            .execute(connection)
            .await?;
    Ok(result.rows_affected())
}

/// Makes a trial user's account the person's own: the email they
/// proved, now, and no longer a trial.
pub async fn set_signed_up(
    connection: &mut PgConnection,
    id: Uuid,
    email: &str,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE accounts_users
         SET email = $2, confirmed_at = $3, trial_expires_at = NULL, updated_at = $3 WHERE id = $1",
    )
    .bind(id)
    .bind(email)
    .bind(now)
    .execute(connection)
    .await?;
    Ok(())
}

/// Deletes an account outright, with its credentials and tokens: an
/// unconfirmed sign-up whose email someone else then proved. Its
/// `app_users` row goes first.
pub async fn delete_row(connection: &mut PgConnection, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM app_users WHERE id = $1")
        .bind(id)
        .execute(&mut *connection)
        .await?;
    sqlx::query("DELETE FROM accounts_users WHERE id = $1")
        .bind(id)
        .execute(connection)
        .await?;
    Ok(())
}

/// The account's Stripe customer, `cus_...`, once it has bought.
#[cfg(feature = "stripe")]
pub async fn get_stripe_customer_id(
    connection: &mut PgConnection,
    id: Uuid,
) -> Result<Option<String>, sqlx::Error> {
    let customer_id: Option<Option<String>> =
        sqlx::query_scalar("SELECT stripe_customer_id FROM accounts_users WHERE id = $1")
            .bind(id)
            .fetch_optional(connection)
            .await?;
    Ok(customer_id.flatten())
}

/// Sets the account's Stripe customer, unless it has one already.
#[cfg(feature = "stripe")]
pub async fn set_stripe_customer_id_if_missing(
    connection: &mut PgConnection,
    id: Uuid,
    customer_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE accounts_users SET stripe_customer_id = $2 WHERE id = $1 AND stripe_customer_id IS NULL",
    )
    .bind(id)
    .bind(customer_id)
    .execute(connection)
    .await?;
    Ok(())
}

/// The account whose Stripe customer it is.
#[cfg(feature = "stripe")]
pub async fn get_id_by_stripe_customer_id(
    connection: &mut PgConnection,
    customer_id: &str,
) -> Result<Option<Uuid>, sqlx::Error> {
    let id: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM accounts_users WHERE stripe_customer_id = $1")
            .bind(customer_id)
            .fetch_optional(connection)
            .await?;
    Ok(id)
}
