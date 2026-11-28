//! Transaction-scoped sync backfills storage. Workflows own policy and commits.

use crate::data::app_sync_state::{from_json, to_json};
use crate::database::Transaction;
use crate::sync::Continuation;
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

/// Queues a community backfill idempotently.
pub(crate) async fn create(tx: &mut Transaction, community_id: Uuid) -> Result<(), sqlx::Error> {
    query!(tx, insert_if_missing(community_id))
}

/// A community the device still has to backfill, with where its
/// backfill got to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Backfill {
    pub community_id: Uuid,
    /// The continuation of the next page, once one page is in.
    pub continuation: Option<Continuation>,
}

/// Lists queued community backfills in community-ID order.
pub(crate) async fn list(tx: &mut Transaction) -> Result<Vec<Backfill>, sqlx::Error> {
    query!(tx, list())?
        .into_iter()
        .map(|(community_id, continuation)| {
            Ok(Backfill {
                community_id,
                continuation: continuation.as_deref().map(from_json).transpose()?,
            })
        })
        .collect()
}

/// Updates where a community's backfill got to, in the same transaction
/// as its applied page.
pub(crate) async fn update_continuation(
    tx: &mut Transaction,
    community_id: Uuid,
    continuation: &Continuation,
) -> Result<(), sqlx::Error> {
    query!(
        tx,
        update_continuation(community_id, &to_json(continuation)?)
    )
}

/// Removes the specified stored record.
pub(crate) async fn delete(tx: &mut Transaction, community_id: Uuid) -> Result<(), sqlx::Error> {
    query!(tx, delete(community_id))
}
