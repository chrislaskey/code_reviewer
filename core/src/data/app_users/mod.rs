//! User records: the `User` schema, its baseline validation, and the
//! reads and writes of `app_users`. Callers own and commit the
//! transaction. Writes are crate-private: a user syncs, so each write
//! goes with its event, see `app::users`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::data::app_sync_changes;
use crate::database::Transaction;
use crate::deployment::Role;
use crate::sync::VersionRange;
use uuid::Uuid;

pub const TABLE: &str = "app_users";
/// The longest name, in characters.
pub const MAX_NAME_LENGTH: usize = 80;

/// A user as the pages show one, with the sync columns every synced row
/// carries, see `docs/design/offline-sync.md`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    /// What the pages show them by.
    pub name: String,
    pub inserted_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub version: i64,
    #[serde(with = "crate::sync::version::optional")]
    pub global_sync_version: Option<i64>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub deleted_by_id: Option<Uuid>,
    /// The public URL of their picture in object storage, see `avatars`.
    #[serde(default)]
    pub avatar_url: Option<String>,
}

/// Why a user may not be written.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum UserInvalid {
    #[error("A name is needed, {MAX_NAME_LENGTH} characters at most")]
    Name,
}

/// A user that passed validation, which every write requires.
pub struct AcceptedUser(User);

impl AcceptedUser {
    /// Checks the user, with their name as it is kept.
    pub fn new(user: User) -> Result<AcceptedUser, UserInvalid> {
        let name = checked_name(&user.name)?;
        Ok(AcceptedUser(User { name, ..user }))
    }

    /// The accepted user.
    pub fn row(&self) -> &User {
        &self.0
    }

    /// The accepted user, owned.
    pub fn into_row(self) -> User {
        self.0
    }
}

/// The name as it is kept: trimmed, not empty and not too long.
pub fn checked_name(name: &str) -> Result<String, UserInvalid> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > MAX_NAME_LENGTH {
        return Err(UserInvalid::Name);
    }
    Ok(name.to_string())
}

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

/// Gets a live profile by ID, preserving the application's existing lookup semantics.
pub async fn get(tx: &mut Transaction, id: Uuid) -> Result<Option<User>, sqlx::Error> {
    Ok(query!(tx, get(id))?.filter(|user| user.deleted_at.is_none()))
}

/// Writes the row over its stored one, a soft delete included, and
/// publishes it on the server as `create` does.
pub(crate) async fn update(
    tx: &mut Transaction,
    role: Role,
    user: AcceptedUser,
) -> Result<User, sqlx::Error> {
    let mut user = user.into_row();
    user.global_sync_version =
        app_sync_changes::stamp(tx, role, TABLE, user.id, user.global_sync_version).await?;
    query!(tx, update(&user))?;
    Ok(user)
}

/// Writes a soft delete over the stored row, and publishes it on the
/// server as `create` does.
pub(crate) async fn delete(
    tx: &mut Transaction,
    role: Role,
    user: AcceptedUser,
) -> Result<User, sqlx::Error> {
    let mut user = user.into_row();
    user.global_sync_version =
        app_sync_changes::stamp(tx, role, TABLE, user.id, user.global_sync_version).await?;
    query!(tx, update_deleted(&user))?;
    Ok(user)
}

/// Inserts a new row. On the server this publishes it: the row gets the
/// next global sync version, recorded in `app_sync_changes` in this
/// transaction. On a device the row keeps the version it has.
pub(crate) async fn create(
    tx: &mut Transaction,
    role: Role,
    user: AcceptedUser,
) -> Result<User, sqlx::Error> {
    let mut user = user.into_row();
    user.global_sync_version =
        app_sync_changes::stamp(tx, role, TABLE, user.id, user.global_sync_version).await?;
    query!(tx, insert(&user))?;
    Ok(user)
}

/// Whether a live user other than the one with the id has the name, in any case.
pub(crate) async fn exists_by_name(
    tx: &mut Transaction,
    name: &str,
    except_id: Uuid,
) -> Result<bool, sqlx::Error> {
    query!(tx, exists_by_name(name, except_id))
}

/// Explicit replication access for internal resource workflows.
pub mod replication {
    use super::*;

    /// Reads protocol state, including tombstones, in the caller transaction.
    pub async fn get(tx: &mut Transaction, id: Uuid) -> Result<Option<User>, sqlx::Error> {
        query!(tx, get(id))
    }

    /// Reads protocol state, including tombstones, in the caller transaction.
    pub async fn list_by_ids(tx: &mut Transaction, ids: &[Uuid]) -> Result<Vec<User>, sqlx::Error> {
        query!(tx, list_by_ids(ids))
    }

    /// Reads protocol state, including tombstones, in the caller transaction.
    pub async fn list_synced_after(
        tx: &mut Transaction,
        user_id: Uuid,
        range: &VersionRange,
        limit: usize,
    ) -> Result<Vec<User>, sqlx::Error> {
        query!(tx, list_synced_after(user_id, range, limit))
    }

    /// Persists a row accepted by the resource workflow.
    pub(crate) async fn upsert(tx: &mut Transaction, user: &User) -> Result<(), sqlx::Error> {
        query!(tx, upsert(user))
    }
}

/// Explicit maintenance access for internal resource workflows.
pub(crate) mod maintenance {
    use super::*;

    /// Deletes every user but the one kept, for a resync from nothing.
    pub(crate) async fn delete_others(tx: &mut Transaction, keep: Uuid) -> Result<(), sqlx::Error> {
        query!(tx, delete_others(keep))
    }

    /// Lists referenced pictures, including deleted users, for byte cleanup.
    #[cfg_attr(not(feature = "object-storage"), allow(dead_code))]
    pub async fn list_avatar_urls(tx: &mut Transaction) -> Result<Vec<String>, sqlx::Error> {
        query!(tx, list_avatar_urls())
    }

    /// Deletes expired unconfirmed users before their accounts, with foreign-key cascades.
    pub(crate) async fn delete_of_unconfirmed_accounts_before(
        tx: &mut Transaction,
        time: DateTime<Utc>,
    ) -> Result<(), sqlx::Error> {
        query!(tx, delete_of_unconfirmed_accounts_before(time))
    }
}

/// Explicit authentication access for internal resource workflows.
pub(crate) mod authentication {
    use super::*;

    /// Resolves an unexpired token hash, retaining tombstones to distinguish deleted accounts.
    pub async fn get_by_token_hash(
        tx: &mut Transaction,
        token_hash: &str,
        api_made_after: DateTime<Utc>,
        session_made_after: DateTime<Utc>,
    ) -> Result<Option<User>, sqlx::Error> {
        query!(
            tx,
            get_by_token_hash(token_hash, api_made_after, session_made_after)
        )
    }
}
