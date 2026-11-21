//! A person's account: their email and when they proved it, one row of
//! `accounts_users` per person. Server only, like every `accounts_`
//! table: an email never reaches another person's device. What the
//! apps sync of a person is `crate::app::users`, a row with the same id,
//! written with this one at sign-up, see `docs/design/multi-app.md`.
//! The email is checked and normalized here, on every write and lookup.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::database::Transaction;
use crate::deployment::now;

#[cfg(feature = "postgres")]
mod postgres;
mod sqlite;

macro_rules! query {
    ($tx:expr, $function:ident($($argument:expr),* $(,)?)) => {
        match $tx {
            Transaction::Sqlite(tx) => sqlite::$function(&mut **tx, $($argument),*).await,
            #[cfg(feature = "postgres")]
            Transaction::Postgres(tx) => postgres::$function(&mut **tx, $($argument),*).await,
        }
    };
}

pub const TABLE: &str = "accounts_users";

/// Phoenix's rule.
pub const MAX_EMAIL_LENGTH: usize = 160;

/// An account as the server keeps it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountUser {
    pub id: Uuid,
    pub email: String,
    /// When the person first proved their email with a code sent to it;
    /// none until then, and such an account has no token.
    pub confirmed_at: Option<DateTime<Utc>>,
    /// When a trial user's 24 hours end; none for every other account,
    /// and again once a trial signs up, see `crate::accounts::trial_users`.
    pub trial_expires_at: Option<DateTime<Utc>>,
    pub inserted_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// Why an account could not be read or written.
#[derive(Debug, Error)]
pub enum AccountUserError {
    #[error("account {id} not found")]
    NotFound { id: Uuid },
    #[error("an email needs an @ and no spaces, {MAX_EMAIL_LENGTH} characters at most")]
    InvalidEmail,
    #[error("that email already has an account")]
    EmailTaken,
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// The live account by its id.
pub async fn get(tx: &mut Transaction, id: Uuid) -> Result<Option<AccountUser>, AccountUserError> {
    Ok(query!(tx, get(id))?.filter(|account| account.deleted_at.is_none()))
}

/// The live account with the email as `checked_email` would keep it.
pub async fn get_by_email(
    tx: &mut Transaction,
    email: &str,
) -> Result<Option<AccountUser>, AccountUserError> {
    Ok(query!(tx, get_by_email(&normalized_email(email)))?)
}

/// The account's row, with the email as `checked_email` keeps it.
pub async fn create(
    tx: &mut Transaction,
    id: Uuid,
    email: &str,
    confirmed: bool,
) -> Result<AccountUser, AccountUserError> {
    create_with_trial(tx, id, email, confirmed, None).await
}

/// A trial user's account: unconfirmed, with when its trial ends.
pub async fn create_trial(
    tx: &mut Transaction,
    id: Uuid,
    email: &str,
    trial_expires_at: DateTime<Utc>,
) -> Result<AccountUser, AccountUserError> {
    create_with_trial(tx, id, email, false, Some(trial_expires_at)).await
}

async fn create_with_trial(
    tx: &mut Transaction,
    id: Uuid,
    email: &str,
    confirmed: bool,
    trial_expires_at: Option<DateTime<Utc>>,
) -> Result<AccountUser, AccountUserError> {
    let now = now();
    let account = AccountUser {
        id,
        email: checked_email(email)?,
        confirmed_at: confirmed.then_some(now),
        trial_expires_at,
        inserted_at: now,
        updated_at: now,
        deleted_at: None,
    };
    query!(tx, insert(&account)).map_err(email_taken)?;
    Ok(account)
}

/// Marks the account's email as proved, now.
pub async fn confirm(tx: &mut Transaction, id: Uuid) -> Result<(), AccountUserError> {
    Ok(query!(tx, set_confirmed_at(id, now()))?)
}

/// Makes a trial user's account the person's own, with the email they
/// just proved: confirmed, and no longer a trial.
pub async fn sign_up_trial(
    tx: &mut Transaction,
    id: Uuid,
    email: &str,
) -> Result<(), AccountUserError> {
    let email = checked_email(email)?;
    query!(tx, set_signed_up(id, &email, now())).map_err(email_taken)
}

/// Marks the account deleted: sign-in finds it no longer, and its
/// email is free to sign up again, see `accounts::deletions`.
pub async fn delete(tx: &mut Transaction, id: Uuid) -> Result<(), AccountUserError> {
    Ok(query!(tx, set_deleted_at(id, now()))?)
}

/// Removes an account and its user outright, with their credentials
/// and tokens: a sign-up that never proved the email someone else then
/// did.
pub async fn delete_unconfirmed(tx: &mut Transaction, id: Uuid) -> Result<(), AccountUserError> {
    Ok(query!(tx, delete_row(id))?)
}

/// The email as it is kept: `normalized_email`, with an @ and no
/// spaces, and not too long.
pub fn checked_email(email: &str) -> Result<String, AccountUserError> {
    let email = normalized_email(email);
    let (before, after) = email
        .split_once('@')
        .ok_or(AccountUserError::InvalidEmail)?;
    if before.is_empty()
        || after.is_empty()
        || email.chars().any(char::is_whitespace)
        || email.chars().count() > MAX_EMAIL_LENGTH
    {
        return Err(AccountUserError::InvalidEmail);
    }
    Ok(email)
}

/// Trimmed and lowercased, so two spellings of one address are one
/// account, one inbox the code goes to, and one key the server limits.
fn normalized_email(email: &str) -> String {
    email.trim().to_lowercase()
}

fn email_taken(error: sqlx::Error) -> AccountUserError {
    match &error {
        sqlx::Error::Database(database) if database.is_unique_violation() => {
            AccountUserError::EmailTaken
        }
        _ => AccountUserError::Database(error),
    }
}

/// Bulk removals for the purge, with their credentials and tokens.
pub(crate) mod maintenance {
    use super::*;

    /// Deletes the accounts that never proved their email and were made
    /// before the time; how many.
    pub(crate) async fn delete_unconfirmed_before(
        tx: &mut Transaction,
        time: DateTime<Utc>,
    ) -> Result<u64, AccountUserError> {
        Ok(query!(tx, delete_unconfirmed_before(time))?)
    }

    /// Deletes the accounts whose user the purge removed; how many.
    pub(crate) async fn delete_without_user(tx: &mut Transaction) -> Result<u64, AccountUserError> {
        Ok(query!(tx, delete_without_user())?)
    }
}

/// Reads that include a deleted account, for lifecycle and billing.
pub mod lifecycle {
    use super::*;

    /// The account, deleted or not.
    pub async fn get(tx: &mut Transaction, id: Uuid) -> Result<Option<AccountUser>, sqlx::Error> {
        query!(tx, get(id))
    }
}

/// The account's Stripe customer.
#[cfg(feature = "stripe")]
pub mod billing {
    use super::*;

    /// The account's Stripe customer id, if it has one.
    pub async fn get_stripe_customer_id(
        tx: &mut Transaction,
        id: Uuid,
    ) -> Result<Option<String>, sqlx::Error> {
        query!(tx, get_stripe_customer_id(id))
    }

    /// Sets the account's Stripe customer id, unless it has one.
    pub async fn set_stripe_customer_id_if_missing(
        tx: &mut Transaction,
        id: Uuid,
        customer_id: &str,
    ) -> Result<(), sqlx::Error> {
        query!(tx, set_stripe_customer_id_if_missing(id, customer_id))
    }

    /// The account with this Stripe customer id.
    pub async fn get_id_by_stripe_customer_id(
        tx: &mut Transaction,
        customer_id: &str,
    ) -> Result<Option<Uuid>, sqlx::Error> {
        query!(tx, get_id_by_stripe_customer_id(customer_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::users;
    use crate::data::accounts_credentials;
    use crate::data::accounts_tokens;
    use crate::database::with_each_database;
    use crate::deployment::Deployment;
    use crate::deployment::Role;
    use crate::ops::staff;

    #[tokio::test]
    async fn accounts_users_signup_records_roll_back_together() {
        with_each_database(|database| async move {
            let deployment = Deployment::new(database, Role::Server);
            let id = Uuid::now_v7();
            let mut tx = deployment.begin().await.expect("transaction");
            create(&mut tx, id, " Alice@Example.com ", false)
                .await
                .expect("account");
            users::create_in(&mut tx, id, "Alice")
                .await
                .expect("profile");
            accounts_credentials::set(&mut tx, id, "correct horse battery")
                .await
                .expect("password");
            let token = accounts_tokens::create(&mut tx, id, accounts_tokens::Context::Api)
                .await
                .expect("token");
            confirm(&mut tx, id).await.expect("confirm");
            assert!(
                get(&mut tx, id)
                    .await
                    .expect("read")
                    .expect("account")
                    .confirmed_at
                    .is_some()
            );
            drop(tx);
            let mut tx = deployment.begin().await.expect("transaction");
            assert!(get(&mut tx, id).await.expect("account").is_none());
            assert!(users::get_in(&mut tx, id).await.expect("profile").is_none());
            assert!(
                !accounts_credentials::is_set(&mut tx, id)
                    .await
                    .expect("password")
            );
            assert!(
                accounts_tokens::get_valid(
                    &mut tx,
                    &accounts_tokens::hash(&token),
                    accounts_tokens::Context::Api,
                    now()
                )
                .await
                .expect("token")
                .is_none()
            );
        })
        .await;
    }

    #[tokio::test]
    async fn accounts_users_cleanup_cascades_credentials_tokens_and_staff_atomically() {
        with_each_database(|database| async move {
            let deployment = Deployment::new(database, Role::Server);
            let id = Uuid::now_v7();
            let mut tx = deployment.begin().await.expect("transaction");
            create(&mut tx, id, "alice@example.com", false)
                .await
                .expect("account");
            let profile = users::create_in(&mut tx, id, "Alice")
                .await
                .expect("profile");
            accounts_credentials::set(&mut tx, id, "correct horse battery")
                .await
                .expect("password");
            let token = accounts_tokens::create(&mut tx, id, accounts_tokens::Context::Api)
                .await
                .expect("token");
            tx.commit().await.expect("commit");
            staff::create(&deployment, "alice@example.com")
                .await
                .expect("staff");
            let mut tx = deployment.begin().await.expect("transaction");
            delete_unconfirmed(&mut tx, id).await.expect("cleanup");
            assert!(
                !accounts_credentials::is_set(&mut tx, id)
                    .await
                    .expect("password")
            );
            drop(tx);
            assert!(staff::get(&deployment, &profile).await.is_ok());
            let mut tx = deployment.begin().await.expect("transaction");
            assert!(
                accounts_credentials::is_set(&mut tx, id)
                    .await
                    .expect("password")
            );
            assert!(
                accounts_tokens::get_valid(
                    &mut tx,
                    &accounts_tokens::hash(&token),
                    accounts_tokens::Context::Api,
                    now()
                )
                .await
                .expect("token")
                .is_some()
            );
            delete_unconfirmed(&mut tx, id).await.expect("cleanup");
            tx.commit().await.expect("commit");
            let mut tx = deployment.begin().await.expect("transaction");
            assert!(get(&mut tx, id).await.expect("account").is_none());
            assert!(users::get_in(&mut tx, id).await.expect("profile").is_none());
            assert!(
                !accounts_credentials::is_set(&mut tx, id)
                    .await
                    .expect("password")
            );
            assert!(
                accounts_tokens::get_valid(
                    &mut tx,
                    &accounts_tokens::hash(&token),
                    accounts_tokens::Context::Api,
                    now()
                )
                .await
                .expect("token")
                .is_none()
            );
            tx.commit().await.expect("commit");
            assert!(matches!(
                staff::get(&deployment, &profile).await,
                Err(crate::ops::staff::StaffError::NotStaff)
            ));
        })
        .await;
    }
}
