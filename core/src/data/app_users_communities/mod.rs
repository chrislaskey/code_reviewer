//! Membership records: the `Membership` schema and the reads and writes of
//! `app_users_communities`. Callers own and commit the transaction. Writes
//! are crate-private: a membership syncs, so each write is an event
//! applied by `app::memberships`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::data::app_sync_changes;
use crate::database::Transaction;
use crate::deployment::Role;
use crate::sync::VersionRange;

pub const TABLE: &str = "app_users_communities";
/// The one role yet.
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
    #[serde(with = "crate::sync::version::optional")]
    pub global_sync_version: Option<i64>,
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

/// Inserts a new row. On the server this publishes it: the row gets the
/// next global sync version, recorded in `app_sync_changes` in this
/// transaction. On a device the row keeps the version it has.
pub(crate) async fn create(
    tx: &mut Transaction,
    role: Role,
    membership: Membership,
) -> Result<Membership, sqlx::Error> {
    let mut membership = membership;
    membership.global_sync_version = app_sync_changes::stamp(
        tx,
        role,
        TABLE,
        membership.id,
        membership.global_sync_version,
    )
    .await?;
    query!(tx, insert(&membership))?;
    Ok(membership)
}

/// Writes the row over its stored one, a soft delete included, and
/// publishes it on the server as `create` does.
pub(crate) async fn update(
    tx: &mut Transaction,
    role: Role,
    membership: Membership,
) -> Result<Membership, sqlx::Error> {
    let mut membership = membership;
    membership.global_sync_version = app_sync_changes::stamp(
        tx,
        role,
        TABLE,
        membership.id,
        membership.global_sync_version,
    )
    .await?;
    query!(tx, update(&membership))?;
    Ok(membership)
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
        range: &VersionRange,
        limit: usize,
    ) -> Result<Vec<Membership>, sqlx::Error> {
        query!(tx, list_synced_after(user_id, range, limit))
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
