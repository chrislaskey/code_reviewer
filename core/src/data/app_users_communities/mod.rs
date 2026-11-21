//! Transaction-scoped membership records. Workflows own commits.

use crate::app::memberships::{AcceptedMembership, Membership};
use crate::app::sync::Cursor;
use crate::database::Transaction;
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

/// The relationship whose live rows are needed by the caller's workflow.
pub(crate) enum Filter {
    User(Uuid),
    Community(Uuid),
}

/// Lists live memberships, oldest first with an ID tie-breaker.
pub(crate) async fn list(
    tx: &mut Transaction,
    filter: Filter,
) -> Result<Vec<Membership>, sqlx::Error> {
    match filter {
        Filter::User(id) => query!(tx, list_live_of_user(id)),
        Filter::Community(id) => query!(tx, list_live_of_community(id)),
    }
}

/// Gets the user's live membership in a community.
pub(crate) async fn get(
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
    membership: AcceptedMembership<'_>,
) -> Result<(), sqlx::Error> {
    query!(tx, insert(membership.row()))
}

/// Persists a row accepted by the resource workflow.
pub(crate) async fn update(
    tx: &mut Transaction,
    membership: AcceptedMembership<'_>,
) -> Result<bool, sqlx::Error> {
    query!(tx, update(membership.row()))
}

/// Explicit replication access for internal resource workflows.
pub(crate) mod replication {
    use super::*;

    /// Reads protocol state, including tombstones, in the caller transaction.
    pub(crate) async fn get(
        tx: &mut Transaction,
        id: Uuid,
    ) -> Result<Option<Membership>, sqlx::Error> {
        query!(tx, get(id))
    }

    /// Reads protocol state, including tombstones, in the caller transaction.
    pub(crate) async fn get_by_user_and_community(
        tx: &mut Transaction,
        user_id: Uuid,
        community_id: Uuid,
    ) -> Result<Option<Membership>, sqlx::Error> {
        query!(tx, get_by_user_and_community(user_id, community_id))
    }

    /// Reads protocol state, including tombstones, in the caller transaction.
    pub(crate) async fn list_synced_after(
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
        membership: AcceptedMembership<'_>,
    ) -> Result<(), sqlx::Error> {
        query!(tx, upsert(membership.row()))
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
