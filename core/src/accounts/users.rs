//! A person's account: their email and when they proved it, one row of
//! `accounts_users` per person. Server only, like every `accounts_`
//! table: an email never reaches another person's device. What the
//! apps sync of a person is `crate::app::users`, a row with the same id,
//! written with this one at sign-up, see `docs/design/multi-app.md`.

use crate::data::accounts_users;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::database::Transaction;
use crate::deployment::{Deployment, now};

/// An account change accepted by this lifecycle workflow.
pub(crate) struct AcceptedUpdate<'a>(accounts_users::Update<'a>);

impl AcceptedUpdate<'_> {
    /// Borrows the accepted lifecycle change.
    pub(crate) fn value(&self) -> &accounts_users::Update<'_> {
        &self.0
    }
}

/// A record accepted by this resource's workflow.
pub(crate) struct AcceptedAccountUser<'a>(&'a AccountUser);

impl AcceptedAccountUser<'_> {
    /// Borrows the accepted record for persistence.
    pub(crate) fn row(&self) -> &AccountUser {
        self.0
    }
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

/// One account by its id, unless deleted.
pub async fn get(deployment: &Deployment, id: Uuid) -> Result<AccountUser, AccountUserError> {
    let mut tx = deployment.begin().await?;
    let account = get_in(&mut tx, id).await?;
    tx.commit().await?;
    account.ok_or(AccountUserError::NotFound { id })
}

/// The account's row, with the email as `checked_email` keeps it.
pub(crate) async fn create_in(
    tx: &mut Transaction,
    id: Uuid,
    email: &str,
    confirmed: bool,
) -> Result<AccountUser, AccountUserError> {
    create_with_trial_in(tx, id, email, confirmed, None).await
}

/// A trial user's account: unconfirmed, with when its trial ends.
pub(crate) async fn create_trial_in(
    tx: &mut Transaction,
    id: Uuid,
    email: &str,
    trial_expires_at: DateTime<Utc>,
) -> Result<AccountUser, AccountUserError> {
    create_with_trial_in(tx, id, email, false, Some(trial_expires_at)).await
}

async fn create_with_trial_in(
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
    accounts_users::create(&mut *tx, AcceptedAccountUser(&account))
        .await
        .map_err(email_taken)?;
    Ok(account)
}

/// Marks the account's email as proved, now.
pub(crate) async fn confirm_in(tx: &mut Transaction, id: Uuid) -> Result<(), AccountUserError> {
    Ok(accounts_users::update(
        &mut *tx,
        id,
        AcceptedUpdate(accounts_users::Update::Confirm(now())),
    )
    .await?)
}

/// Makes a trial user's account the person's own, with the email they
/// just proved: confirmed, and no longer a trial.
pub(crate) async fn sign_up_trial_in(
    tx: &mut Transaction,
    id: Uuid,
    email: &str,
) -> Result<(), AccountUserError> {
    accounts_users::update(
        &mut *tx,
        id,
        AcceptedUpdate(accounts_users::Update::SignUp { email, at: now() }),
    )
    .await
    .map_err(email_taken)
}

/// Removes an account and its user outright: a sign-up that never
/// proved the email someone else then did.
pub(crate) async fn delete_unconfirmed_in(
    tx: &mut Transaction,
    id: Uuid,
) -> Result<(), AccountUserError> {
    Ok(accounts_users::maintenance::delete(&mut *tx, id).await?)
}

/// Marks the account deleted: sign-in finds it no longer, and its
/// email is free to sign up again, see `accounts::deletions`.
pub(crate) async fn delete_in(tx: &mut Transaction, id: Uuid) -> Result<(), AccountUserError> {
    Ok(accounts_users::delete(&mut *tx, id, now()).await?)
}

/// The live account by its id.
pub(crate) async fn get_in(
    tx: &mut Transaction,
    id: Uuid,
) -> Result<Option<AccountUser>, AccountUserError> {
    Ok(accounts_users::get(tx, id).await?)
}

/// The live account with the email as `checked_email` would keep it.
pub(crate) async fn get_by_email_in(
    tx: &mut Transaction,
    email: &str,
) -> Result<Option<AccountUser>, AccountUserError> {
    Ok(accounts_users::get_by_email(&mut *tx, &normalized_email(email)).await?)
}

/// Deletes the accounts that never proved their email and were made
/// before the time, with their credentials and tokens, for the purge;
/// how many.
pub(crate) async fn delete_unconfirmed_before(
    tx: &mut Transaction,
    time: DateTime<Utc>,
) -> Result<u64, AccountUserError> {
    Ok(accounts_users::maintenance::delete_unconfirmed_before(&mut *tx, time).await?)
}

/// Deletes the accounts whose user the purge removed, with their
/// credentials and tokens; how many.
pub(crate) async fn delete_without_user(tx: &mut Transaction) -> Result<u64, AccountUserError> {
    Ok(accounts_users::maintenance::delete_without_user(&mut *tx).await?)
}

/// The email as it is kept: `normalized_email`, with an @ and no
/// spaces, and not too long.
pub(crate) fn checked_email(email: &str) -> Result<String, AccountUserError> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::users;
    use crate::database::with_each_database;
    use crate::deployment::Role;
    use crate::ops::staff;
    use crate::{credentials, tokens};

    #[tokio::test]
    async fn accounts_users_signup_records_roll_back_together() {
        with_each_database(|database| async move {
            let deployment = Deployment::new(database, Role::Server);
            let id = Uuid::now_v7();
            let mut tx = deployment.begin().await.expect("transaction");
            create_in(&mut tx, id, " Alice@Example.com ", false)
                .await
                .expect("account");
            users::create_in(&mut tx, id, "Alice")
                .await
                .expect("profile");
            credentials::set(&mut tx, id, "correct horse battery")
                .await
                .expect("password");
            let token = tokens::create(&mut tx, id, tokens::Context::Api)
                .await
                .expect("token");
            confirm_in(&mut tx, id).await.expect("confirm");
            assert!(
                get_in(&mut tx, id)
                    .await
                    .expect("read")
                    .expect("account")
                    .confirmed_at
                    .is_some()
            );
            drop(tx);
            let mut tx = deployment.begin().await.expect("transaction");
            assert!(get_in(&mut tx, id).await.expect("account").is_none());
            assert!(users::get_in(&mut tx, id).await.expect("profile").is_none());
            assert!(!credentials::is_set(&mut tx, id).await.expect("password"));
            assert!(
                tokens::get_valid(&mut tx, &tokens::hash(&token), tokens::Context::Api, now())
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
            create_in(&mut tx, id, "alice@example.com", false)
                .await
                .expect("account");
            let profile = users::create_in(&mut tx, id, "Alice")
                .await
                .expect("profile");
            credentials::set(&mut tx, id, "correct horse battery")
                .await
                .expect("password");
            let token = tokens::create(&mut tx, id, tokens::Context::Api)
                .await
                .expect("token");
            tx.commit().await.expect("commit");
            staff::create(&deployment, "alice@example.com")
                .await
                .expect("staff");
            let mut tx = deployment.begin().await.expect("transaction");
            delete_unconfirmed_in(&mut tx, id).await.expect("cleanup");
            assert!(!credentials::is_set(&mut tx, id).await.expect("password"));
            drop(tx);
            assert!(staff::get(&deployment, &profile).await.is_ok());
            let mut tx = deployment.begin().await.expect("transaction");
            assert!(credentials::is_set(&mut tx, id).await.expect("password"));
            assert!(
                tokens::get_valid(&mut tx, &tokens::hash(&token), tokens::Context::Api, now())
                    .await
                    .expect("token")
                    .is_some()
            );
            delete_unconfirmed_in(&mut tx, id).await.expect("cleanup");
            tx.commit().await.expect("commit");
            let mut tx = deployment.begin().await.expect("transaction");
            assert!(get_in(&mut tx, id).await.expect("account").is_none());
            assert!(users::get_in(&mut tx, id).await.expect("profile").is_none());
            assert!(!credentials::is_set(&mut tx, id).await.expect("password"));
            assert!(
                tokens::get_valid(&mut tx, &tokens::hash(&token), tokens::Context::Api, now())
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
