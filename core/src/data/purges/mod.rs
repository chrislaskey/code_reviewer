//! Transaction-scoped purges storage. Workflows own policy and commits.

use crate::database::Transaction;
use crate::purge::AcceptedPurge;
use chrono::{DateTime, Utc};

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

/// Business tables whose tombstones may be purged, children first at the call site.
#[derive(Clone, Copy)]
pub(crate) enum Table {
    PostsCommunities,
    Posts,
    Files,
    Memberships,
    Communities,
    Users,
}

impl Table {
    /// Returns the fixed table name; callers cannot supply SQL identifiers.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::PostsCommunities => crate::posts_communities::TABLE,
            Self::Posts => crate::posts::TABLE,
            Self::Files => crate::files::TABLE,
            Self::Memberships => crate::memberships::TABLE,
            Self::Communities => crate::communities::TABLE,
            Self::Users => crate::users::TABLE,
        }
    }
}

/// Persists the record accepted by its workflow.
pub(crate) async fn create(
    tx: &mut Transaction,
    purge: AcceptedPurge<'_>,
) -> Result<(), sqlx::Error> {
    query!(tx, insert(purge.row()))
}

/// Gets the most recent purge horizon, or none before the first purge.
pub(crate) async fn get(tx: &mut Transaction) -> Result<Option<DateTime<Utc>>, sqlx::Error> {
    query!(tx, latest_horizon())
}

/// Explicit maintenance operations; bulk SQL and cascades do not run per-row hooks.
pub(crate) mod maintenance {
    use super::*;

    /// Physically removes retained tombstones from an allowed table.
    pub(crate) async fn delete_deleted_before(
        tx: &mut Transaction,
        table: Table,
        horizon: DateTime<Utc>,
    ) -> Result<u64, sqlx::Error> {
        query!(tx, delete_deleted_before(table.name(), horizon))
    }
}
