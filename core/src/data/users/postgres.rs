//! The `app_users` table, the counterpart of `sqlite::users` with native
//! uuid and timestamptz columns.

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgConnection};
use uuid::Uuid;

use crate::sync::Cursor;
use crate::users::User;

#[derive(FromRow)]
struct UserRow {
    id: Uuid,
    name: String,
    inserted_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    version: i64,
    last_synced_at: Option<DateTime<Utc>>,
    deleted_at: Option<DateTime<Utc>>,
    deleted_by_id: Option<Uuid>,
    avatar_url: Option<String>,
}

impl From<UserRow> for User {
    fn from(row: UserRow) -> User {
        User {
            id: row.id,
            name: row.name,
            inserted_at: row.inserted_at,
            updated_at: row.updated_at,
            version: row.version,
            last_synced_at: row.last_synced_at,
            deleted_at: row.deleted_at,
            deleted_by_id: row.deleted_by_id,
            avatar_url: row.avatar_url,
        }
    }
}

macro_rules! select {
    ($($rest:expr),+ $(,)?) => {
        concat!(
            "SELECT id, name, inserted_at, updated_at, version, last_synced_at, deleted_at, deleted_by_id, avatar_url FROM app_users ",
            $($rest),+
        )
    };
}

/// The row by its id, deleted or not.
pub async fn get(connection: &mut PgConnection, id: Uuid) -> Result<Option<User>, sqlx::Error> {
    let row: Option<UserRow> = sqlx::query_as(select!("WHERE id = $1"))
        .bind(id)
        .fetch_optional(connection)
        .await?;
    Ok(row.map(User::from))
}

/// The user the token with the hash stands for, deleted or not: an app's token
/// made after `api_made_after`, or the browser's made after
/// `session_made_after`, which is when each would have expired.
pub async fn get_by_token_hash(
    connection: &mut PgConnection,
    token_hash: &str,
    api_made_after: DateTime<Utc>,
    session_made_after: DateTime<Utc>,
) -> Result<Option<User>, sqlx::Error> {
    let row: Option<UserRow> = sqlx::query_as(
        "SELECT u.id, u.name, u.inserted_at, u.updated_at, u.version, u.last_synced_at, u.deleted_at, u.deleted_by_id, u.avatar_url
         FROM app_users u JOIN accounts_tokens t ON t.user_id = u.id
         WHERE t.token_hash = $1
           AND ((t.context = 'api' AND t.inserted_at > $2) OR (t.context = 'session' AND t.inserted_at > $3))",
    )
    .bind(token_hash)
    .bind(api_made_after)
    .bind(session_made_after)
    .fetch_optional(connection)
    .await?;
    Ok(row.map(User::from))
}

/// Writes the fields a user's event may change, with the sync columns.
pub async fn update(connection: &mut PgConnection, user: &User) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE app_users SET name = $2, avatar_url = $3, updated_at = $4, version = $5, last_synced_at = $6
         WHERE id = $1",
    )
    .bind(user.id)
    .bind(&user.name)
    .bind(&user.avatar_url)
    .bind(user.updated_at)
    .bind(user.version)
    .bind(user.last_synced_at)
    .execute(connection)
    .await?;
    Ok(())
}

/// Writes a deleted user: the fields `update` writes and the delete's.
pub async fn update_deleted(connection: &mut PgConnection, user: &User) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE app_users SET name = $2, avatar_url = $3, updated_at = $4, version = $5, last_synced_at = $6,
           deleted_at = $7, deleted_by_id = $8
         WHERE id = $1",
    )
    .bind(user.id)
    .bind(&user.name)
    .bind(&user.avatar_url)
    .bind(user.updated_at)
    .bind(user.version)
    .bind(user.last_synced_at)
    .bind(user.deleted_at)
    .bind(user.deleted_by_id)
    .execute(connection)
    .await?;
    Ok(())
}

/// Every user's picture, deleted users' too, for the sweep.
#[cfg_attr(not(feature = "object-storage"), allow(dead_code))]
pub async fn list_avatar_urls(connection: &mut PgConnection) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar("SELECT avatar_url FROM app_users WHERE avatar_url IS NOT NULL")
        .fetch_all(connection)
        .await
}

/// Deletes the users whose account never proved its email and was made
/// before the time, ahead of the accounts themselves, see
/// `accounts_users::delete_unconfirmed_before`.
pub async fn delete_of_unconfirmed_accounts_before(
    connection: &mut PgConnection,
    time: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "DELETE FROM app_users WHERE id IN
           (SELECT id FROM accounts_users
            WHERE confirmed_at IS NULL AND trial_expires_at IS NULL AND inserted_at < $1)",
    )
    .bind(time)
    .execute(connection)
    .await?;
    Ok(())
}

/// The users the user, `$1`, sees: themselves, and the live members of
/// the communities they are a live member of.
macro_rules! visible_to {
    () => {
        "(id = $1 OR id IN (
            SELECT uc.user_id FROM app_users_communities uc
            WHERE uc.deleted_at IS NULL AND uc.community_id IN (SELECT community_id FROM app_users_communities WHERE user_id = $1 AND deleted_at IS NULL)))"
    };
}

/// The rows with the ids, deleted or not.
pub async fn list_by_ids(
    connection: &mut PgConnection,
    ids: &[Uuid],
) -> Result<Vec<User>, sqlx::Error> {
    let rows: Vec<UserRow> = sqlx::query_as(select!("WHERE id = ANY($1)"))
        .bind(ids)
        .fetch_all(connection)
        .await?;
    Ok(rows.into_iter().map(User::from).collect())
}

/// The rows the server wrote after the cursor that the user sees, in
/// cursor order, deleted or not: a page of a pull.
pub async fn list_synced_after(
    connection: &mut PgConnection,
    user_id: Uuid,
    after: &Cursor,
    limit: usize,
) -> Result<Vec<User>, sqlx::Error> {
    let rows: Vec<UserRow> = sqlx::query_as(select!(
        "WHERE last_synced_at IS NOT NULL AND (last_synced_at, id) > ($2, $3) AND ",
        visible_to!(),
        " ORDER BY last_synced_at, id LIMIT $4"
    ))
    .bind(user_id)
    .bind(after.last_synced_at)
    .bind(after.id)
    .bind(i64::try_from(limit).unwrap_or(i64::MAX))
    .fetch_all(connection)
    .await?;
    Ok(rows.into_iter().map(User::from).collect())
}

pub async fn insert(connection: &mut PgConnection, user: &User) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO app_users (id, name, inserted_at, updated_at, version, last_synced_at, deleted_at, deleted_by_id, avatar_url)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
    )
    .bind(user.id)
    .bind(&user.name)
    .bind(user.inserted_at)
    .bind(user.updated_at)
    .bind(user.version)
    .bind(user.last_synced_at)
    .bind(user.deleted_at)
    .bind(user.deleted_by_id)
    .bind(&user.avatar_url)
    .execute(connection)
    .await?;
    Ok(())
}

/// Writes the server's row over the device's, or inserts it.
pub async fn upsert(connection: &mut PgConnection, user: &User) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO app_users (id, name, inserted_at, updated_at, version, last_synced_at, deleted_at, deleted_by_id, avatar_url)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
         ON CONFLICT (id) DO UPDATE SET
           name = excluded.name, inserted_at = excluded.inserted_at, updated_at = excluded.updated_at,
           version = excluded.version, last_synced_at = excluded.last_synced_at,
           deleted_at = excluded.deleted_at, deleted_by_id = excluded.deleted_by_id,
           avatar_url = excluded.avatar_url",
    )
    .bind(user.id)
    .bind(&user.name)
    .bind(user.inserted_at)
    .bind(user.updated_at)
    .bind(user.version)
    .bind(user.last_synced_at)
    .bind(user.deleted_at)
    .bind(user.deleted_by_id)
    .bind(&user.avatar_url)
    .execute(connection)
    .await?;
    Ok(())
}
