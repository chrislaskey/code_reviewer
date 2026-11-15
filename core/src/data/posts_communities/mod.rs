//! Transaction-scoped post-community association records. Workflows own commits.

use crate::database::Transaction;
use crate::posts_communities::{AcceptedPostCommunity, PostCommunity};
use crate::sync::Cursor;
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

/// A required relationship scope for internal association reads.
pub(crate) enum Filter<'a> {
    Posts(&'a [Uuid]),
    Membership(Uuid),
}

/// Lists live association rows within the workflow's relationship scope.
pub(crate) async fn list(
    tx: &mut Transaction,
    filter: Filter<'_>,
) -> Result<Vec<PostCommunity>, sqlx::Error> {
    match filter {
        Filter::Posts([]) => Ok(Vec::new()),
        Filter::Posts(ids) => query!(tx, list_live_of_posts(ids)),
        Filter::Membership(id) => query!(tx, list_live_of_membership(id)),
    }
}

/// Persists a row accepted by the resource workflow.
pub(crate) async fn create(
    tx: &mut Transaction,
    row: AcceptedPostCommunity<'_>,
) -> Result<(), sqlx::Error> {
    query!(tx, insert(row.row()))
}

/// Persists a row accepted by the resource workflow.
pub(crate) async fn update(
    tx: &mut Transaction,
    row: AcceptedPostCommunity<'_>,
) -> Result<bool, sqlx::Error> {
    query!(tx, update(row.row()))
}

/// Explicit replication access for internal resource workflows.
pub(crate) mod replication {
    use super::*;

    /// Reads protocol state, including tombstones, in the caller transaction.
    pub(crate) async fn get(
        tx: &mut Transaction,
        id: Uuid,
    ) -> Result<Option<PostCommunity>, sqlx::Error> {
        query!(tx, get(id))
    }

    /// Reads protocol state, including tombstones, in the caller transaction.
    pub(crate) async fn get_by_post_and_community(
        tx: &mut Transaction,
        post_id: Uuid,
        community_id: Uuid,
    ) -> Result<Option<PostCommunity>, sqlx::Error> {
        query!(tx, get_by_post_and_community(post_id, community_id))
    }

    /// Reads protocol state, including tombstones, in the caller transaction.
    pub(crate) async fn list_of_membership_deleted_at(
        tx: &mut Transaction,
        membership_id: Uuid,
        deleted_at: DateTime<Utc>,
    ) -> Result<Vec<PostCommunity>, sqlx::Error> {
        query!(tx, list_of_membership_deleted_at(membership_id, deleted_at))
    }

    /// Reads protocol state, including tombstones, in the caller transaction.
    pub(crate) async fn list_synced_after(
        tx: &mut Transaction,
        user_id: Uuid,
        after: &Cursor,
        limit: usize,
    ) -> Result<Vec<PostCommunity>, sqlx::Error> {
        query!(tx, list_synced_after(user_id, after, limit))
    }

    /// Persists a row accepted by the resource workflow.
    pub(crate) async fn upsert(
        tx: &mut Transaction,
        row: AcceptedPostCommunity<'_>,
    ) -> Result<(), sqlx::Error> {
        query!(tx, upsert(row.row()))
    }

    /// Lists live association rows for a membership-authorized backfill.
    pub(crate) async fn list_live_of_community_after(
        tx: &mut Transaction,
        community_id: Uuid,
        after: &Cursor,
        limit: usize,
    ) -> Result<Vec<PostCommunity>, sqlx::Error> {
        query!(tx, list_live_of_community_after(community_id, after, limit))
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
