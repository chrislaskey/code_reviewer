//! Transaction-scoped trial users storage. Workflows own policy and commits.

use crate::database::Transaction;
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

/// Counts current trial accounts for the server capacity policy.
pub(crate) async fn count(tx: &mut Transaction) -> Result<i64, sqlx::Error> {
    query!(tx, count())
}

/// Counts the trial user's writes for the existing trial limit.
pub(crate) async fn count_writes(tx: &mut Transaction, user_id: Uuid) -> Result<i64, sqlx::Error> {
    query!(tx, count_writes(user_id))
}

/// Checks the trial isolation rule for joining a community.
pub(crate) async fn join_is_walled_off(
    tx: &mut Transaction,
    user_id: Uuid,
    community_id: Uuid,
) -> Result<bool, sqlx::Error> {
    query!(tx, join_is_walled_off(user_id, community_id))
}

/// Checks the trial isolation rule for viewing a community.
pub(crate) async fn community_is_hidden_from(
    tx: &mut Transaction,
    community_id: Uuid,
    viewer_id: Uuid,
) -> Result<bool, sqlx::Error> {
    query!(tx, community_is_hidden_from(community_id, viewer_id))
}

/// Explicit maintenance operations; bulk SQL and cascades do not run per-row hooks.
pub(crate) mod maintenance {
    use super::*;

    /// Marks a promoted trial's communities changed for the next pull.
    pub(crate) async fn touch_communities_of(
        tx: &mut Transaction,
        user_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<(), sqlx::Error> {
        query!(tx, touch_communities_of(user_id, now))
    }

    /// Removes one trial and its related rows through the existing bulk cleanup.
    pub(crate) async fn delete_one(
        tx: &mut Transaction,
        user_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        query!(tx, delete_one(user_id))
    }

    /// Removes expired trial accounts and related rows at the policy cutoff.
    pub(crate) async fn delete_expired_before(
        tx: &mut Transaction,
        time: DateTime<Utc>,
    ) -> Result<u64, sqlx::Error> {
        query!(tx, delete_expired_before(time))
    }
}
