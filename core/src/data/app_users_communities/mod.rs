//! Membership records: the `Membership` schema and the reads and writes of
//! `app_users_communities`. Callers own and commit the transaction. Writes
//! are crate-private: a membership syncs, so each write is an event
//! applied by `app::memberships`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::app::sync::Cursor;
use crate::database::Transaction;

pub const TABLE: &str = "app_users_communities";
pub const ROLE_MEMBER: &str = "member";

/// A user's membership in a community, with the sync columns every
/// synced row carries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Membership {
    pub id: Uuid,
    pub user_id: Uuid,
    pub community_id: Uuid,
    pub role: String,
    pub inserted_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub version: i64,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub deleted_by_id: Option<Uuid>,
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

/// The relationship whose live rows are needed by the caller's workflow.
pub enum Filter {
    User(Uuid),
    Community(Uuid),
}

/// Lists live memberships, oldest first with an ID tie-breaker.
pub async fn list(tx: &mut Transaction, filter: Filter) -> Result<Vec<Membership>, sqlx::Error> {
    match filter {
        Filter::User(id) => query!(tx, list_live_of_user(id)),
        Filter::Community(id) => query!(tx, list_live_of_community(id)),
    }
}

/// Gets the user's live membership in a community.
pub async fn get(
    tx: &mut Transaction,
    user_id: Uuid,
    community_id: Uuid,
) -> Result<Option<Membership>, sqlx::Error> {
    Ok(
        query!(tx, get_by_user_and_community(user_id, community_id))?
            .filter(|row| row.deleted_at.is_none()),
    )
}

/// Persists a row accepted by the resource workflow.
pub(crate) async fn create(
    tx: &mut Transaction,
    membership: &Membership,
) -> Result<(), sqlx::Error> {
    query!(tx, insert(membership))
}

/// Persists a row accepted by the resource workflow.
pub(crate) async fn update(
    tx: &mut Transaction,
    membership: &Membership,
) -> Result<bool, sqlx::Error> {
    query!(tx, update(membership))
}

/// Explicit replication access for internal resource workflows.
pub mod replication {
    use super::*;

    /// Reads protocol state, including tombstones, in the caller transaction.
    pub async fn get(tx: &mut Transaction, id: Uuid) -> Result<Option<Membership>, sqlx::Error> {
        query!(tx, get(id))
    }

    /// Reads protocol state, including tombstones, in the caller transaction.
    pub async fn get_by_user_and_community(
        tx: &mut Transaction,
        user_id: Uuid,
        community_id: Uuid,
    ) -> Result<Option<Membership>, sqlx::Error> {
        query!(tx, get_by_user_and_community(user_id, community_id))
    }

    /// Reads protocol state, including tombstones, in the caller transaction.
    pub async fn list_synced_after(
        tx: &mut Transaction,
        user_id: Uuid,
        after: &Cursor,
        limit: usize,
    ) -> Result<Vec<Membership>, sqlx::Error> {
        query!(tx, list_synced_after(user_id, after, limit))
    }

    /// Persists a row accepted by the resource workflow.
    pub(crate) async fn upsert(
        tx: &mut Transaction,
        membership: &Membership,
    ) -> Result<(), sqlx::Error> {
        query!(tx, upsert(membership))
    }
}

/// Explicit maintenance access for internal resource workflows.
pub(crate) mod maintenance {
    use super::*;

    /// Physically removes a refused local row and cascading children.
    pub(crate) async fn delete(tx: &mut Transaction, id: Uuid) -> Result<(), sqlx::Error> {
        query!(tx, delete_row(id))
    }
}
