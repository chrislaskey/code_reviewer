//! Transaction-scoped token records. Workflows own commits.

use crate::database::Transaction;
use crate::tokens::Context;
use crate::tokens::{AcceptedToken, Token};
use chrono::{DateTime, Utc};
use uuid::Uuid;

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

/// Persists a token whose secrets were hashed by the token workflow.
pub(crate) async fn create(
    tx: &mut Transaction,
    token: AcceptedToken<'_>,
) -> Result<(), sqlx::Error> {
    query!(tx, insert(token.row()))
}

/// Gets a token with the required context and strict expiry cutoff.
pub(crate) async fn get(
    tx: &mut Transaction,
    token_hash: &str,
    context: Context,
    made_after: DateTime<Utc>,
) -> Result<Option<Token>, sqlx::Error> {
    query!(tx, get_by_hash(token_hash, context, made_after))
}

/// Atomically increments and returns attempts; never uses a read-then-write sequence.
pub(crate) async fn increment_attempts(tx: &mut Transaction, id: Uuid) -> Result<i64, sqlx::Error> {
    query!(tx, increment_attempts(id))
}

/// Replaces a hashed code and resets its attempts and expiry timestamp.
pub(crate) async fn update_code(
    tx: &mut Transaction,
    id: Uuid,
    code_hash: crate::tokens::AcceptedCodeHash<'_>,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    query!(tx, set_code(id, code_hash.value(), now))
}

/// Removes one token by ID.
pub(crate) async fn delete(tx: &mut Transaction, id: Uuid) -> Result<(), sqlx::Error> {
    query!(tx, delete_by_id(id))
}

/// Removes all of a user's tokens in the required context.
pub(crate) async fn delete_of_user(
    tx: &mut Transaction,
    user_id: Uuid,
    context: Context,
) -> Result<u64, sqlx::Error> {
    query!(tx, delete_of_user(user_id, context))
}

/// Removes an API or browser session token by hash; other contexts are untouched.
pub(crate) async fn delete_signed_in_by_hash(
    tx: &mut Transaction,
    token_hash: &str,
) -> Result<bool, sqlx::Error> {
    query!(tx, delete_signed_in_by_hash(token_hash))
}

/// Explicit maintenance access for internal workflows.
pub(crate) mod maintenance {
    use super::*;

    /// Deletes expired tokens in one context using its workflow cutoff.
    pub(crate) async fn delete_made_before(
        tx: &mut Transaction,
        context: Context,
        time: DateTime<Utc>,
    ) -> Result<u64, sqlx::Error> {
        query!(tx, delete_made_before(context, time))
    }
}
