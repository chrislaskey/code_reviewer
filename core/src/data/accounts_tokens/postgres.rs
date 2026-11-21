//! The `accounts_tokens` table, the counterpart of `sqlite::tokens`.

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgConnection};
use uuid::Uuid;

use crate::tokens::{Context, Token};

#[derive(FromRow)]
struct TokenRow {
    id: Uuid,
    user_id: Uuid,
    context: String,
    token_hash: String,
    code_hash: Option<String>,
    attempts: i32,
    sent_to: Option<String>,
    inserted_at: DateTime<Utc>,
}

impl TryFrom<TokenRow> for Token {
    type Error = sqlx::Error;

    fn try_from(row: TokenRow) -> Result<Token, sqlx::Error> {
        let context = Context::parse(&row.context).ok_or_else(|| {
            sqlx::Error::Decode(format!("unknown token context '{}'", row.context).into())
        })?;
        Ok(Token {
            id: row.id,
            user_id: row.user_id,
            context,
            token_hash: row.token_hash,
            code_hash: row.code_hash,
            attempts: i64::from(row.attempts),
            sent_to: row.sent_to,
            inserted_at: row.inserted_at,
        })
    }
}

pub async fn insert(connection: &mut PgConnection, token: &Token) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO accounts_tokens (id, user_id, context, token_hash, code_hash, attempts, sent_to, inserted_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(token.id)
    .bind(token.user_id)
    .bind(token.context.as_str())
    .bind(&token.token_hash)
    .bind(&token.code_hash)
    .bind(i32::try_from(token.attempts).unwrap_or(i32::MAX))
    .bind(&token.sent_to)
    .bind(token.inserted_at)
    .execute(connection)
    .await?;
    Ok(())
}

/// The token with the hash in the context, if made after `made_after`.
pub async fn get_by_hash(
    connection: &mut PgConnection,
    token_hash: &str,
    context: Context,
    made_after: DateTime<Utc>,
) -> Result<Option<Token>, sqlx::Error> {
    let row: Option<TokenRow> = sqlx::query_as(
        "SELECT id, user_id, context, token_hash, code_hash, attempts, sent_to, inserted_at FROM accounts_tokens
         WHERE token_hash = $1 AND context = $2 AND inserted_at > $3",
    )
    .bind(token_hash)
    .bind(context.as_str())
    .bind(made_after)
    .fetch_optional(connection)
    .await?;
    row.map(Token::try_from).transpose()
}

/// Counts one more attempt; the count after.
pub async fn increment_attempts(
    connection: &mut PgConnection,
    id: Uuid,
) -> Result<i64, sqlx::Error> {
    let row: (i32,) = sqlx::query_as(
        "UPDATE accounts_tokens SET attempts = attempts + 1 WHERE id = $1 RETURNING attempts",
    )
    .bind(id)
    .fetch_one(connection)
    .await?;
    Ok(i64::from(row.0))
}

/// A new code for the pending sign-in, made now, with no attempts.
pub async fn set_code(
    connection: &mut PgConnection,
    id: Uuid,
    code_hash: &str,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE accounts_tokens SET code_hash = $2, inserted_at = $3, attempts = 0 WHERE id = $1",
    )
    .bind(id)
    .bind(code_hash)
    .bind(now)
    .execute(connection)
    .await?;
    Ok(())
}

pub async fn delete_by_id(connection: &mut PgConnection, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM accounts_tokens WHERE id = $1")
        .bind(id)
        .execute(connection)
        .await?;
    Ok(())
}

pub async fn delete_of_user(
    connection: &mut PgConnection,
    user_id: Uuid,
    context: Context,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query("DELETE FROM accounts_tokens WHERE user_id = $1 AND context = $2")
        .bind(user_id)
        .bind(context.as_str())
        .execute(connection)
        .await?;
    Ok(result.rows_affected())
}

/// Deletes an app's or the browser's token with the hash; whether there
/// was one.
pub async fn delete_signed_in_by_hash(
    connection: &mut PgConnection,
    token_hash: &str,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "DELETE FROM accounts_tokens WHERE token_hash = $1 AND context IN ('api', 'session')",
    )
    .bind(token_hash)
    .execute(connection)
    .await?;
    Ok(result.rows_affected() > 0)
}

/// Deletes the tokens of the context made before the time, which have
/// expired.
pub async fn delete_made_before(
    connection: &mut PgConnection,
    context: Context,
    time: DateTime<Utc>,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query("DELETE FROM accounts_tokens WHERE context = $1 AND inserted_at < $2")
        .bind(context.as_str())
        .bind(time)
        .execute(connection)
        .await?;
    Ok(result.rows_affected())
}
