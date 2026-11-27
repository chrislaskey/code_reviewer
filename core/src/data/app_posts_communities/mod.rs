//! Post community records: the `PostCommunity` schema and the reads and
//! writes of `app_posts_communities`. Callers own and commit the
//! transaction. Writes are crate-private: a post community row syncs, so
//! each write is an event applied by `app::posts_communities`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::data::app_sync_changes;
use crate::database::Transaction;
use crate::deployment::Role;
use crate::sync::VersionRange;

pub const TABLE: &str = "app_posts_communities";

/// A post in a community, with the sync columns every synced row carries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PostCommunity {
    pub id: Uuid,
    pub post_id: Uuid,
    pub community_id: Uuid,

    /// The owner's membership the post was added under.
    pub users_communities_id: Uuid,
    pub inserted_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub version: i64,
    pub last_synced_at: Option<DateTime<Utc>>,
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

/// A required relationship scope for internal association reads.
pub enum Filter<'a> {
    Posts(&'a [Uuid]),
    Membership(Uuid),
}

/// Lists live association rows within the workflow's relationship scope.
pub async fn list(
    tx: &mut Transaction,
    filter: Filter<'_>,
) -> Result<Vec<PostCommunity>, sqlx::Error> {
    match filter {
        Filter::Posts([]) => Ok(Vec::new()),
        Filter::Posts(ids) => query!(tx, list_live_of_posts(ids)),
        Filter::Membership(id) => query!(tx, list_live_of_membership(id)),
    }
}

/// Inserts a new row. On the server this publishes it: the row gets the
/// next global sync version, recorded in `app_sync_changes` in this
/// transaction. On a device the row keeps the version it has.
pub(crate) async fn create(
    tx: &mut Transaction,
    role: Role,
    row: PostCommunity,
) -> Result<PostCommunity, sqlx::Error> {
    let mut row = row;
    row.global_sync_version =
        app_sync_changes::stamp(tx, role, TABLE, row.id, row.global_sync_version).await?;
    query!(tx, insert(&row))?;
    Ok(row)
}

/// Writes the row over its stored one, a soft delete included, and
/// publishes it on the server as `create` does.
pub(crate) async fn update(
    tx: &mut Transaction,
    role: Role,
    row: PostCommunity,
) -> Result<PostCommunity, sqlx::Error> {
    let mut row = row;
    row.global_sync_version =
        app_sync_changes::stamp(tx, role, TABLE, row.id, row.global_sync_version).await?;
    query!(tx, update(&row))?;
    Ok(row)
}

/// Explicit replication access for internal resource workflows.
pub mod replication {
    use super::*;

    /// Reads protocol state, including tombstones, in the caller transaction.
    pub async fn get(tx: &mut Transaction, id: Uuid) -> Result<Option<PostCommunity>, sqlx::Error> {
        query!(tx, get(id))
    }

    /// Reads protocol state, including tombstones, in the caller transaction.
    pub async fn get_by_post_and_community(
        tx: &mut Transaction,
        post_id: Uuid,
        community_id: Uuid,
    ) -> Result<Option<PostCommunity>, sqlx::Error> {
        query!(tx, get_by_post_and_community(post_id, community_id))
    }

    /// Reads protocol state, including tombstones, in the caller transaction.
    pub async fn list_of_membership_deleted_at(
        tx: &mut Transaction,
        membership_id: Uuid,
        deleted_at: DateTime<Utc>,
    ) -> Result<Vec<PostCommunity>, sqlx::Error> {
        query!(tx, list_of_membership_deleted_at(membership_id, deleted_at))
    }

    /// Reads protocol state, including tombstones, in the caller transaction.
    pub async fn list_synced_after(
        tx: &mut Transaction,
        user_id: Uuid,
        range: &VersionRange,
        limit: usize,
    ) -> Result<Vec<PostCommunity>, sqlx::Error> {
        query!(tx, list_synced_after(user_id, range, limit))
    }

    /// Persists a row accepted by the resource workflow.
    pub(crate) async fn upsert(
        tx: &mut Transaction,
        row: &PostCommunity,
    ) -> Result<(), sqlx::Error> {
        query!(tx, upsert(row))
    }

    /// Lists live association rows for a membership-authorized backfill.
    pub async fn list_live_of_community_after(
        tx: &mut Transaction,
        community_id: Uuid,
        range: &VersionRange,
        limit: usize,
    ) -> Result<Vec<PostCommunity>, sqlx::Error> {
        query!(tx, list_live_of_community_after(community_id, range, limit))
    }
}

/// Explicit maintenance access for internal resource workflows.
pub(crate) mod maintenance {
    use super::*;

    /// Physically removes a refused local row and cascading children.
    pub(crate) async fn delete(tx: &mut Transaction, id: Uuid) -> Result<(), sqlx::Error> {
        query!(tx, delete_row(id))
    }

    /// Moves every post from one community to another, through the
    /// membership given, when a local user's communities are merged; a
    /// post already in both keeps only its row in the second.
    pub(crate) async fn move_rows_to_community(
        tx: &mut Transaction,
        from_community_id: Uuid,
        to_community_id: Uuid,
        to_membership_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        query!(
            tx,
            move_rows_to_community(from_community_id, to_community_id, to_membership_id)
        )
    }
}
