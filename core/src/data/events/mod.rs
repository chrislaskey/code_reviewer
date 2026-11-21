//! Transaction-scoped events storage. Workflows own policy and commits.

use crate::app::events::{AcceptedEvent, Conflicts, EventStatus, StoredEvent};
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

/// A required scope for queue and log lists.
pub(crate) enum Filter<'a> {
    Status(EventStatus),
    Table(&'a str),
}

/// Lists events in the required scope, in their existing deterministic order.
pub(crate) async fn list(
    tx: &mut Transaction,
    filter: Filter<'_>,
) -> Result<Vec<StoredEvent>, sqlx::Error> {
    match filter {
        Filter::Status(status) => query!(tx, list_by_status(status)),
        Filter::Table(table) => query!(tx, list_by_table(table)),
    }
}

/// Persists the record accepted by its workflow.
pub(crate) async fn create(
    tx: &mut Transaction,
    event: AcceptedEvent<'_>,
) -> Result<(), sqlx::Error> {
    query!(tx, insert(event.row()))
}

/// Gets one stored record in the caller transaction.
pub(crate) async fn get(
    tx: &mut Transaction,
    id: Uuid,
) -> Result<Option<StoredEvent>, sqlx::Error> {
    query!(tx, get(id))
}

/// Counts events in the required state.
pub(crate) async fn count(tx: &mut Transaction, status: EventStatus) -> Result<i64, sqlx::Error> {
    query!(tx, count_by_status(status))
}

/// Checks whether a row has a queued change.
pub(crate) async fn has_pending(
    tx: &mut Transaction,
    table: &str,
    row_id: Uuid,
) -> Result<bool, sqlx::Error> {
    query!(tx, has_pending(table, row_id))
}

/// Removes the specified stored record.
pub(crate) async fn delete(tx: &mut Transaction, id: Uuid) -> Result<(), sqlx::Error> {
    query!(tx, delete(id))
}

/// Removes pending events for a row while retaining conflicts.
pub(crate) async fn delete_pending_of_row(
    tx: &mut Transaction,
    table: &str,
    row_id: Uuid,
) -> Result<(), sqlx::Error> {
    query!(tx, delete_pending_of_row(table, row_id))
}

/// Removes all events for the specified resource row.
pub(crate) async fn delete_of_row(
    tx: &mut Transaction,
    table: &str,
    row_id: Uuid,
) -> Result<(), sqlx::Error> {
    query!(tx, delete_of_row(table, row_id))
}

/// Explicit processing operations; bulk SQL and cascades do not run per-row hooks.
pub(crate) mod processing {
    use super::*;

    /// Stores conflict state and fields using the existing transition.
    pub(crate) async fn mark_conflict(
        tx: &mut Transaction,
        id: Uuid,
        conflicts: &Conflicts,
    ) -> Result<(), sqlx::Error> {
        query!(tx, mark_conflict(id, conflicts))
    }

    /// Atomically increments attempts and records the refusal reason.
    pub(crate) async fn add_attempt(
        tx: &mut Transaction,
        id: Uuid,
        reason: &str,
    ) -> Result<i64, sqlx::Error> {
        query!(tx, add_attempt(id, reason))
    }

    /// Stores the terminal refusal as a conflict with its reason.
    pub(crate) async fn mark_refused(
        tx: &mut Transaction,
        id: Uuid,
        conflicts: &Conflicts,
        reason: &str,
    ) -> Result<(), sqlx::Error> {
        query!(tx, mark_refused(id, conflicts, reason))
    }

    /// Restores pending state and clears the existing retry metadata.
    pub(crate) async fn mark_pending(tx: &mut Transaction, id: Uuid) -> Result<(), sqlx::Error> {
        query!(tx, mark_pending(id))
    }

    /// Records a failure reason without counting a refusal attempt.
    pub(crate) async fn mark_error(
        tx: &mut Transaction,
        id: Uuid,
        reason: &str,
    ) -> Result<(), sqlx::Error> {
        query!(tx, mark_error(id, reason))
    }
}

/// Explicit maintenance operations; bulk SQL and cascades do not run per-row hooks.
pub(crate) mod maintenance {
    use super::*;

    /// Removes events for business rows that no longer exist.
    pub(crate) async fn delete_of_missing_rows(tx: &mut Transaction) -> Result<u64, sqlx::Error> {
        query!(tx, delete_of_missing_rows())
    }
}
