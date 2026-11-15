//! Passwords: a user's password hash in `accounts_credentials`, server
//! only. A password is optional, a second factor checked after the
//! emailed code, see `accounts`; a user who made none has no row here.
//! Argon2id with the crate's defaults; hashing and checking run off the
//! async runtime's threads, since each takes a moment by design.

use crate::data::credentials as credentials_records;
use argon2::Argon2;
use argon2::password_hash::{PasswordHasher, PasswordVerifier};
use thiserror::Error;
use uuid::Uuid;

use crate::database::Transaction;
use crate::deployment::now;

/// A password hash produced by this module after validation and hashing.
pub(crate) struct AcceptedPasswordHash<'a>(&'a str);

impl AcceptedPasswordHash<'_> {
    /// Borrows the encoded hash for persistence.
    pub(crate) fn value(&self) -> &str {
        self.0
    }
}

/// Phoenix's rule: long enough to be worth hashing, and bounded.
pub const MIN_PASSWORD_LENGTH: usize = 12;
pub const MAX_PASSWORD_LENGTH: usize = 72;

/// The hash of a password nobody has: checked against when the user
/// has none, so a check takes as long either way.
const NOBODYS_HASH: &str = "$argon2id$v=19$m=19456,t=2,p=1$VDToes2uMjjUIjjyc6cYUw$osBqQQ/i0dGiz8Z96SZjgzMpXKhznbnS4HD6Z+h5NX4";

#[derive(Debug, Error)]
pub enum CredentialError {
    #[error("a password needs at least {MIN_PASSWORD_LENGTH} characters")]
    PasswordTooShort,
    #[error("a password may have at most {MAX_PASSWORD_LENGTH} characters")]
    PasswordTooLong,
    #[error("could not hash the password: {0}")]
    Hash(String),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// Keeps the user's password, as its hash, in place of any they had.
pub(crate) async fn set(
    tx: &mut Transaction,
    user_id: Uuid,
    password: &str,
) -> Result<(), CredentialError> {
    let password_hash = checked_hash(password).await?;
    credentials_records::upsert(
        &mut *tx,
        user_id,
        AcceptedPasswordHash(&password_hash),
        now(),
    )
    .await?;
    Ok(())
}

/// Whether the user has a password.
pub(crate) async fn is_set(tx: &mut Transaction, user_id: Uuid) -> Result<bool, CredentialError> {
    Ok(credentials_records::get(&mut *tx, user_id).await?.is_some())
}

/// The hash of the password, once checked for length.
async fn checked_hash(password: &str) -> Result<String, CredentialError> {
    let password = checked_password(password)?;
    off_the_runtime(move || {
        Argon2::default()
            .hash_password(password.as_bytes())
            .map(|hash| hash.to_string())
            .map_err(|error| CredentialError::Hash(error.to_string()))
    })
    .await
}

/// Whether the password is the user's; false for a user with no
/// password, and for no user at all, after the same work either way.
pub(crate) async fn verify(
    tx: &mut Transaction,
    user_id: Option<Uuid>,
    password: &str,
) -> Result<bool, CredentialError> {
    let password_hash = match user_id {
        Some(user_id) => credentials_records::get(&mut *tx, user_id).await?,
        None => None,
    };
    let nobody = password_hash.is_none();
    let password_hash = password_hash.unwrap_or_else(|| NOBODYS_HASH.to_string());
    let password = password.to_string();
    let matches = off_the_runtime(move || {
        Ok(Argon2::default()
            .verify_password(password.as_bytes(), password_hash.as_str())
            .is_ok())
    })
    .await?;
    Ok(matches && !nobody)
}

fn checked_password(password: &str) -> Result<String, CredentialError> {
    let length = password.chars().count();
    if length < MIN_PASSWORD_LENGTH {
        return Err(CredentialError::PasswordTooShort);
    }
    if length > MAX_PASSWORD_LENGTH || password.len() > MAX_PASSWORD_LENGTH {
        return Err(CredentialError::PasswordTooLong);
    }
    Ok(password.to_string())
}

async fn off_the_runtime<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, CredentialError> + Send + 'static,
) -> Result<T, CredentialError> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| CredentialError::Hash(error.to_string()))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::with_each_database;
    use crate::deployment::{Deployment, Role};
    use crate::users;

    #[tokio::test]
    async fn credentials_set_and_verify_and_set_again() {
        with_each_database(|database| async move {
            let deployment = Deployment::new(database, Role::Server);
            let alice = users::create(&deployment, "alice@example.com", "Alice")
                .await
                .expect("a user");
            let mut tx = deployment.begin().await.expect("a transaction");
            set(&mut tx, alice.id, "correct horse battery")
                .await
                .expect("kept");
            assert!(
                verify(&mut tx, Some(alice.id), "correct horse battery")
                    .await
                    .expect("checked")
            );
            set(&mut tx, alice.id, "another horse battery")
                .await
                .expect("replaced");
            assert!(
                verify(&mut tx, Some(alice.id), "another horse battery")
                    .await
                    .expect("checked")
            );
            assert!(
                !verify(&mut tx, Some(alice.id), "correct horse battery")
                    .await
                    .expect("checked"),
                "the old one is gone"
            );
            assert!(
                !verify(&mut tx, Some(alice.id), "wrong horse battery")
                    .await
                    .expect("checked")
            );
            assert!(
                !verify(&mut tx, None, "correct horse battery")
                    .await
                    .expect("checked")
            );
            tx.commit().await.expect("committed");
        })
        .await;
    }

    #[tokio::test]
    async fn credentials_create_refuses_a_short_or_long_password() {
        with_each_database(|database| async move {
            let deployment = Deployment::new(database, Role::Server);
            let alice = users::create(&deployment, "alice@example.com", "Alice")
                .await
                .expect("a user");
            let mut tx = deployment.begin().await.expect("a transaction");
            let error = set(&mut tx, alice.id, "short")
                .await
                .expect_err("too short");
            assert!(matches!(error, CredentialError::PasswordTooShort));
            let error = set(&mut tx, alice.id, &"x".repeat(73))
                .await
                .expect_err("too long");
            assert!(matches!(error, CredentialError::PasswordTooLong));
            tx.commit().await.expect("committed");
        })
        .await;
    }

    #[test]
    fn credentials_nobodys_hash_is_a_real_hash_matching_nothing_anyone_types() {
        let parsed =
            argon2::password_hash::phc::PasswordHash::new(NOBODYS_HASH).expect("a PHC string");
        assert!(parsed.hash.is_some());
        assert!(Argon2::default().verify_password(b"", &parsed).is_err());
        assert!(
            Argon2::default()
                .verify_password(b"correct horse battery", &parsed)
                .is_err()
        );
    }
}
