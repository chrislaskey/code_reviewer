//! Transaction-scoped jobs storage. Workflows own policy and commits.

use crate::app::jobs::{Job, JobRecord};
use crate::database::Transaction;
use chrono::{DateTime, Utc};
use serde_json::Value;

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

/// Gets one stored record in the caller transaction.
pub(crate) async fn get(tx: &mut Transaction, job: Job) -> Result<Option<JobRecord>, sqlx::Error> {
    query!(tx, get(job))
}

/// Explicit processing operations; bulk SQL and cascades do not run per-row hooks.
pub(crate) mod processing {
    use super::*;

    /// Atomically claims a due job; false if another caller already claimed it.
    pub(crate) async fn claim_if_due(
        tx: &mut Transaction,
        job: Job,
        now: DateTime<Utc>,
        due_after: DateTime<Utc>,
    ) -> Result<bool, sqlx::Error> {
        query!(tx, claim_if_due(job, now, due_after))
    }

    /// Records a forced job start in the caller transaction.
    pub(crate) async fn claim(
        tx: &mut Transaction,
        job: Job,
        now: DateTime<Utc>,
    ) -> Result<(), sqlx::Error> {
        query!(tx, claim(job, now))
    }

    /// Stores a job completion report or failure in the caller transaction.
    pub(crate) async fn finish(
        tx: &mut Transaction,
        job: Job,
        finished_at: DateTime<Utc>,
        last_report: Option<&Value>,
        last_error: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        query!(tx, finish(job, finished_at, last_report, last_error))
    }
}
