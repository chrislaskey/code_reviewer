//! Job records: the `JobRecord` schema, one row per job by its name, and
//! the reads and writes of `app_jobs`. Callers own and commit the
//! transaction. When each job runs, and where, is `jobs`.

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::database::Transaction;

/// A job's last run, as `app_jobs` keeps it.
#[derive(Clone, Debug, PartialEq)]
pub struct JobRecord {
    pub name: String,
    pub started_at: DateTime<Utc>,
    /// None while it runs, or when it stopped before recording its end.
    pub finished_at: Option<DateTime<Utc>>,
    pub last_report: Option<Value>,
    pub last_error: Option<String>,
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

/// Gets one stored record in the caller transaction.
pub async fn get(tx: &mut Transaction, name: &str) -> Result<Option<JobRecord>, sqlx::Error> {
    query!(tx, get(name))
}

/// Explicit processing operations; bulk SQL and cascades do not run per-row hooks.
pub mod processing {
    use super::*;

    /// Atomically claims a due job; false if another caller already claimed it.
    pub async fn claim_if_due(
        tx: &mut Transaction,
        name: &str,
        now: DateTime<Utc>,
        due_after: DateTime<Utc>,
    ) -> Result<bool, sqlx::Error> {
        query!(tx, claim_if_due(name, now, due_after))
    }

    /// Records a forced job start in the caller transaction.
    pub async fn claim(
        tx: &mut Transaction,
        name: &str,
        now: DateTime<Utc>,
    ) -> Result<(), sqlx::Error> {
        query!(tx, claim(name, now))
    }

    /// Stores a job completion report or failure in the caller transaction.
    pub async fn finish(
        tx: &mut Transaction,
        name: &str,
        finished_at: DateTime<Utc>,
        last_report: Option<&Value>,
        last_error: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        query!(tx, finish(name, finished_at, last_report, last_error))
    }
}
