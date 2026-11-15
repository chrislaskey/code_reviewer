//! Transaction-scoped user records. Workflows own validation and commits.

use crate::database::Transaction;
use crate::sync::Cursor;
use crate::users::{AcceptedUser, User};
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

/// Gets a live profile by ID, preserving the application's existing lookup semantics.
pub(crate) async fn get(tx: &mut Transaction, id: Uuid) -> Result<Option<User>, sqlx::Error> {
    Ok(query!(tx, get(id))?.filter(|user| user.deleted_at.is_none()))
}

/// Persists a row accepted by the resource workflow.
pub(crate) async fn update(
    tx: &mut Transaction,
    user: AcceptedUser<'_>,
) -> Result<(), sqlx::Error> {
    query!(tx, update(user.row()))
}

/// Persists a row accepted by the resource workflow.
pub(crate) async fn delete(
    tx: &mut Transaction,
    user: AcceptedUser<'_>,
) -> Result<(), sqlx::Error> {
    query!(tx, update_deleted(user.row()))
}

/// Persists a row accepted by the resource workflow.
pub(crate) async fn create(
    tx: &mut Transaction,
    user: AcceptedUser<'_>,
) -> Result<(), sqlx::Error> {
    query!(tx, insert(user.row()))
}

/// Explicit replication access for internal resource workflows.
pub(crate) mod replication {
    use super::*;

    /// Reads protocol state, including tombstones, in the caller transaction.
    pub(crate) async fn get(tx: &mut Transaction, id: Uuid) -> Result<Option<User>, sqlx::Error> {
        query!(tx, get(id))
    }

    /// Reads protocol state, including tombstones, in the caller transaction.
    pub(crate) async fn list_by_ids(
        tx: &mut Transaction,
        ids: &[Uuid],
    ) -> Result<Vec<User>, sqlx::Error> {
        query!(tx, list_by_ids(ids))
    }

    /// Reads protocol state, including tombstones, in the caller transaction.
    pub(crate) async fn list_synced_after(
        tx: &mut Transaction,
        user_id: Uuid,
        after: &Cursor,
        limit: usize,
    ) -> Result<Vec<User>, sqlx::Error> {
        query!(tx, list_synced_after(user_id, after, limit))
    }

    /// Persists a row accepted by the resource workflow.
    pub(crate) async fn upsert(
        tx: &mut Transaction,
        user: AcceptedUser<'_>,
    ) -> Result<(), sqlx::Error> {
        query!(tx, upsert(user.row()))
    }
}

/// Explicit maintenance access for internal resource workflows.
pub(crate) mod maintenance {
    use super::*;

    /// Lists referenced pictures, including deleted users, for byte cleanup.
    #[cfg_attr(not(feature = "object-storage"), allow(dead_code))]
    pub(crate) async fn list_avatar_urls(tx: &mut Transaction) -> Result<Vec<String>, sqlx::Error> {
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
    pub(crate) async fn get_by_token_hash(
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
