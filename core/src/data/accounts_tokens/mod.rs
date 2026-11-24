//! Tokens: every secret the server hands out, one row of `accounts_tokens`
//! each, kept as its hash so reading the table gives nothing that works,
//! and told apart by its `Context`, which also sets how long it lives.
//! The API token an app sends as a bearer token on every request and
//! the token the browser's cookie holds, which the server resolves to a
//! user, `users::get_by_token`; the pending sign-in that waits for its
//! emailed code, which carries the code's hash and the wrong attempts
//! so far; the one whose code matched and that waits for its last step;
//! and the device code a finished
//! browser sign-in leaves for the device to collect, see `accounts`;
//! and the emailed code that confirms deleting an account, see
//! `accounts::deletions`.
//! Made and deleted there; the expired ones go with the purge.

use chrono::{DateTime, Duration, Utc};
use uuid::Uuid;

use crate::database::Transaction;
use crate::deployment::now;
use crate::helpers::{random, sha256};

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

/// How long an app's token works, as Phoenix's API tokens do.
pub const API_VALIDITY_DAYS: i64 = 365;
/// How long the browser's token works, as Phoenix's sessions do.
pub const SESSION_VALIDITY_DAYS: i64 = 60;
/// How long a sign-in waits for its code.
pub const SIGN_IN_CODE_VALIDITY_MINUTES: i64 = 10;
/// How long a device has to collect its code.
pub const DEVICE_CODE_VALIDITY_MINUTES: i64 = 5;

/// What a token is for, which sets how long it lives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Context {
    /// An app's bearer token, a year.
    Api,
    /// The browser's cookie, sixty days.
    Session,
    /// A sign-in that waits for its emailed code, ten minutes.
    SignInCode,
    /// A sign-in whose code matched and that waits for its last step,
    /// ten minutes.
    SignInConfirm,
    /// A finished browser sign-in waiting for the device to collect it,
    /// five minutes.
    DeviceCode,
    /// The emailed code that confirms deleting the account, ten minutes.
    DeleteCode,
}

impl Context {
    /// Every context.
    pub const ALL: [Context; 6] = [
        Context::Api,
        Context::Session,
        Context::SignInCode,
        Context::SignInConfirm,
        Context::DeviceCode,
        Context::DeleteCode,
    ];

    /// The context as the column keeps it.
    pub fn as_str(self) -> &'static str {
        match self {
            Context::Api => "api",
            Context::Session => "session",
            Context::SignInCode => "sign_in_code",
            Context::SignInConfirm => "sign_in_confirm",
            Context::DeviceCode => "device_code",
            Context::DeleteCode => "delete_code",
        }
    }

    /// The context with this name, if there is one.
    pub fn parse(context: &str) -> Option<Context> {
        Context::ALL
            .into_iter()
            .find(|candidate| candidate.as_str() == context)
    }

    /// How long a token of this context works.
    pub fn validity(self) -> Duration {
        match self {
            Context::Api => Duration::days(API_VALIDITY_DAYS),
            Context::Session => Duration::days(SESSION_VALIDITY_DAYS),
            Context::SignInCode | Context::SignInConfirm | Context::DeleteCode => {
                Duration::minutes(SIGN_IN_CODE_VALIDITY_MINUTES)
            }
            Context::DeviceCode => Duration::minutes(DEVICE_CODE_VALIDITY_MINUTES),
        }
    }

    /// The time a token of this context must have been made after to be
    /// valid at `now`.
    pub fn made_after_to_be_valid(self, now: DateTime<Utc>) -> DateTime<Utc> {
        now - self.validity()
    }
}

/// A token as `accounts_tokens` keeps it: by its hash.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    pub id: Uuid,
    pub user_id: Uuid,
    pub context: Context,
    pub token_hash: String,
    /// The emailed code's hash, for a pending sign-in.
    pub code_hash: Option<String>,
    /// Wrong codes so far, for a pending sign-in.
    pub attempts: i64,
    /// The address the code went to when it is not yet the account's:
    /// a trial user signing up, see `accounts::sign_in_from_trial`.
    pub sent_to: Option<String>,
    pub inserted_at: DateTime<Utc>,
}

/// Makes a token for the user, an app's or the browser's, and returns
/// it: 32 random bytes as hex, kept as its hash and matched exactly.
pub async fn create(
    tx: &mut Transaction,
    user_id: Uuid,
    context: Context,
) -> Result<String, sqlx::Error> {
    let token = random::hex(32);
    insert(tx, user_id, context, hash(&token), None, None).await?;
    Ok(token)
}

/// Makes the user's pending sign-in, replacing any earlier one, and
/// returns the pending token the page holds and the six-digit code to
/// email them, both kept as hashes; with the address the code goes to
/// when it is not the account's own.
pub async fn create_sign_in_code(
    tx: &mut Transaction,
    user_id: Uuid,
    sent_to: Option<&str>,
) -> Result<(String, String), sqlx::Error> {
    delete_of_user(tx, user_id, Context::SignInCode).await?;
    let pending = random::hex(32);
    let code = random::digits(6);
    insert(
        tx,
        user_id,
        Context::SignInCode,
        hash(&pending),
        Some(hash(&code)),
        sent_to,
    )
    .await?;
    Ok((pending, code))
}

/// Makes the pending token of the user's sign-in whose code matched,
/// replacing any earlier one, for its last step; with the address the
/// code went to when it is not the account's own.
pub async fn create_sign_in_confirm(
    tx: &mut Transaction,
    user_id: Uuid,
    sent_to: Option<&str>,
) -> Result<String, sqlx::Error> {
    delete_of_user(tx, user_id, Context::SignInConfirm).await?;
    let pending = random::hex(32);
    insert(
        tx,
        user_id,
        Context::SignInConfirm,
        hash(&pending),
        None,
        sent_to,
    )
    .await?;
    Ok(pending)
}

/// Makes the device code for the user's finished browser sign-in: 16
/// random bytes as hex, kept as the hash of the code and the device's
/// challenge together, so the row is found only by someone holding
/// both, see `accounts::sign_in_with_device_code`.
pub async fn create_device_code(
    tx: &mut Transaction,
    user_id: Uuid,
    challenge: &str,
) -> Result<String, sqlx::Error> {
    let code = random::hex(16);
    insert(
        tx,
        user_id,
        Context::DeviceCode,
        device_code_hash(&code, challenge),
        None,
        None,
    )
    .await?;
    Ok(code)
}

/// Makes the user's code to confirm deleting their account, replacing
/// any earlier one: the six digits to email, kept as a hash on a row
/// found by the user, `delete_code_hash`.
pub async fn create_delete_code(
    tx: &mut Transaction,
    user_id: Uuid,
) -> Result<String, sqlx::Error> {
    delete_of_user(tx, user_id, Context::DeleteCode).await?;
    let code = random::digits(6);
    insert(
        tx,
        user_id,
        Context::DeleteCode,
        delete_code_hash(user_id),
        Some(hash(&code)),
        None,
    )
    .await?;
    Ok(code)
}

/// The hash the user's delete code row is kept under.
pub fn delete_code_hash(user_id: Uuid) -> String {
    hash(&format!("delete_code:{user_id}"))
}

/// Gives the pending sign-in a new code, its ten minutes starting over
/// and its wrong attempts forgotten; the code to email.
pub async fn reset_code(tx: &mut Transaction, id: Uuid) -> Result<String, sqlx::Error> {
    let code = random::digits(6);
    query!(&mut *tx, set_code(id, &hash(&code), now()))?;
    Ok(code)
}

/// The token with the hash in the context, if it has not expired as of
/// `now`.
pub async fn get_valid(
    tx: &mut Transaction,
    token_hash: &str,
    context: Context,
    now: DateTime<Utc>,
) -> Result<Option<Token>, sqlx::Error> {
    query!(
        tx,
        get_by_hash(token_hash, context, context.made_after_to_be_valid(now))
    )
}

/// Counts one more wrong code against the pending sign-in; the attempts
/// so far, this one included.
pub async fn count_attempt(tx: &mut Transaction, id: Uuid) -> Result<i64, sqlx::Error> {
    query!(tx, increment_attempts(id))
}

/// Deletes one token by its id.
pub async fn delete_by_id(tx: &mut Transaction, id: Uuid) -> Result<(), sqlx::Error> {
    query!(tx, delete_by_id(id))
}

/// Deletes the user's tokens of the context.
pub async fn delete_of_user(
    tx: &mut Transaction,
    user_id: Uuid,
    context: Context,
) -> Result<u64, sqlx::Error> {
    query!(tx, delete_of_user(user_id, context))
}

/// Deletes an app's or the browser's token, so it no longer stands for
/// anyone; whether it did.
pub async fn delete(tx: &mut Transaction, token: &str) -> Result<bool, sqlx::Error> {
    query!(tx, delete_signed_in_by_hash(&hash(token)))
}

/// Deletes the tokens that have expired as of `now`, each context by its
/// own validity, for the purge.
pub async fn delete_expired(tx: &mut Transaction, now: DateTime<Utc>) -> Result<u64, sqlx::Error> {
    let mut deleted = 0;
    for context in Context::ALL {
        deleted += query!(
            &mut *tx,
            delete_made_before(context, context.made_after_to_be_valid(now))
        )?;
    }
    Ok(deleted)
}

/// The time an app's token must have been made after to be valid at
/// `now`, and the browser's.
pub fn made_after_to_be_valid(now: DateTime<Utc>) -> (DateTime<Utc>, DateTime<Utc>) {
    (
        Context::Api.made_after_to_be_valid(now),
        Context::Session.made_after_to_be_valid(now),
    )
}

/// The hash kept for a token: SHA-256 as hex.
pub fn hash(token: &str) -> String {
    sha256::hex(token.as_bytes())
}

/// The hash a device code row is kept under: of the code and the
/// device's challenge together.
pub fn device_code_hash(code: &str, challenge: &str) -> String {
    hash(&format!("{code}{challenge}"))
}

async fn insert(
    tx: &mut Transaction,
    user_id: Uuid,
    context: Context,
    token_hash: String,
    code_hash: Option<String>,
    sent_to: Option<&str>,
) -> Result<(), sqlx::Error> {
    let row = Token {
        id: Uuid::now_v7(),
        user_id,
        context,
        token_hash,
        code_hash,
        attempts: 0,
        sent_to: sent_to.map(str::to_string),
        inserted_at: now(),
    };
    query!(tx, insert(&row))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::users::{self, UserError};
    use crate::database::with_each_database;
    use crate::deployment::{Deployment, Role};

    #[tokio::test]
    async fn tokens_reset_code_preserves_pending_token_and_restarts_attempts() {
        with_each_database(|database| async move {
            let deployment = Deployment::new(database, Role::Server);
            let alice = users::create(&deployment, "alice@example.com", "Alice")
                .await
                .expect("user");
            let mut tx = deployment.begin().await.expect("transaction");
            let (pending, _) =
                create_sign_in_code(&mut tx, alice.id, Some("destination@example.com"))
                    .await
                    .expect("code");
            let original = get_valid(&mut tx, &hash(&pending), Context::SignInCode, now())
                .await
                .expect("read")
                .expect("token");
            assert_eq!(
                count_attempt(&mut tx, original.id).await.expect("attempt"),
                1
            );
            assert_eq!(
                count_attempt(&mut tx, original.id).await.expect("attempt"),
                2
            );
            let code = reset_code(&mut tx, original.id).await.expect("reset");
            let reset = get_valid(&mut tx, &hash(&pending), Context::SignInCode, now())
                .await
                .expect("read")
                .expect("token");
            assert_eq!(reset.id, original.id);
            assert_eq!(reset.token_hash, original.token_hash);
            assert_eq!(reset.sent_to, original.sent_to);
            assert_eq!(reset.attempts, 0);
            assert_eq!(reset.code_hash, Some(hash(&code)));
            assert!(reset.inserted_at >= original.inserted_at);
            assert!(
                get_valid(&mut tx, &hash(&pending), Context::SignInConfirm, now())
                    .await
                    .expect("other context")
                    .is_none()
            );
            assert_eq!(
                count_attempt(&mut tx, original.id).await.expect("attempt"),
                1
            );
        })
        .await;
    }

    #[tokio::test]
    async fn tokens_create_stands_for_its_user_until_deleted() {
        with_each_database(|database| async move {
            let deployment = Deployment::new(database, Role::Server);
            let alice = users::create(&deployment, "alice@example.com", "Alice")
                .await
                .expect("a user");
            let mut tx = deployment.begin().await.expect("a transaction");
            let token = create(&mut tx, alice.id, Context::Api)
                .await
                .expect("a token");
            let session = create(&mut tx, alice.id, Context::Session)
                .await
                .expect("a session token");
            tx.commit().await.expect("committed");
            assert_eq!(token.len(), 64);

            for token in [&token, &session] {
                let user = users::get_by_token(&deployment, token)
                    .await
                    .expect("the user");
                assert_eq!(user, alice);
            }
            let error = users::get_by_token(&deployment, "not-a-token")
                .await
                .expect_err("unknown");
            assert!(matches!(error, UserError::UnknownToken));

            let mut tx = deployment.begin().await.expect("a transaction");
            assert!(delete(&mut tx, &token).await.expect("deleted"));
            assert!(!delete(&mut tx, &token).await.expect("already gone"));
            tx.commit().await.expect("committed");
            let error = users::get_by_token(&deployment, &token)
                .await
                .expect_err("deleted");
            assert!(matches!(error, UserError::UnknownToken));
        })
        .await;
    }

    #[tokio::test]
    async fn tokens_are_kept_hashed_and_expire_each_by_its_context() {
        with_each_database(|database| async move {
            let deployment = Deployment::new(database, Role::Server);
            let alice = users::create(&deployment, "alice@example.com", "Alice")
                .await
                .expect("a user");
            let mut tx = deployment.begin().await.expect("a transaction");
            let token = create(&mut tx, alice.id, Context::Api)
                .await
                .expect("a token");
            let session = create(&mut tx, alice.id, Context::Session)
                .await
                .expect("a session token");
            let (pending, code) = create_sign_in_code(&mut tx, alice.id, None)
                .await
                .expect("a pending sign-in");
            let device_code = create_device_code(&mut tx, alice.id, "the challenge")
                .await
                .expect("a device code");
            tx.commit().await.expect("committed");
            assert_eq!(code.len(), 6);
            assert!(code.chars().all(|c| c.is_ascii_digit()));
            assert_eq!(device_code.len(), 32);
            assert_ne!(hash(&token), token);

            let mut tx = deployment.begin().await.expect("a transaction");
            let row = get_valid(&mut tx, &hash(&pending), Context::SignInCode, now())
                .await
                .expect("looked")
                .expect("the pending sign-in");
            assert_eq!(row.code_hash, Some(hash(&code)));
            assert!(
                get_valid(&mut tx, &hash(&pending), Context::Api, now())
                    .await
                    .expect("looked")
                    .is_none(),
                "a pending token never passes as an API token"
            );
            assert!(
                get_valid(
                    &mut tx,
                    &device_code_hash(&device_code, "the challenge"),
                    Context::DeviceCode,
                    now()
                )
                .await
                .expect("looked")
                .is_some()
            );
            assert!(
                get_valid(
                    &mut tx,
                    &device_code_hash(&device_code, "another challenge"),
                    Context::DeviceCode,
                    now()
                )
                .await
                .expect("looked")
                .is_none(),
                "not without the right challenge"
            );
            assert_eq!(count_attempt(&mut tx, row.id).await.expect("one"), 1);
            assert_eq!(count_attempt(&mut tx, row.id).await.expect("two"), 2);
            let new_code = reset_code(&mut tx, row.id).await.expect("a new code");
            let row = get_valid(&mut tx, &hash(&pending), Context::SignInCode, now())
                .await
                .expect("looked")
                .expect("still the pending sign-in");
            assert_eq!(row.code_hash, Some(hash(&new_code)));
            assert_eq!(row.attempts, 0, "forgotten with the new code");

            assert_eq!(delete_expired(&mut tx, now()).await.expect("none"), 0);
            let in_an_hour = now() + Duration::hours(1);
            assert_eq!(
                delete_expired(&mut tx, in_an_hour).await.expect("two"),
                2,
                "the code and the device code"
            );
            let in_a_season = now() + Duration::days(SESSION_VALIDITY_DAYS + 1);
            assert_eq!(delete_expired(&mut tx, in_a_season).await.expect("one"), 1);
            let in_a_year = now() + Duration::days(API_VALIDITY_DAYS + 1);
            assert_eq!(delete_expired(&mut tx, in_a_year).await.expect("one"), 1);
            tx.commit().await.expect("committed");
            assert!(users::get_by_token(&deployment, &token).await.is_err());
            assert!(users::get_by_token(&deployment, &session).await.is_err());
        })
        .await;
    }
}
