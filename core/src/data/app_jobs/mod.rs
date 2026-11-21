//! Job records: the `Job` named by each row, the `JobRecord` schema, and
//! the reads and writes of `app_jobs`. Callers own and commit the
//! transaction. When each job runs, and where, is `app::jobs`.

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::database::Transaction;

/// A recurring job, one row of `app_jobs` each, see `app::jobs`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Job {
    /// `purge::run`, before the sweeps, so the files of the rows it
    /// removes go in the same day.
    Purge,
    /// `trial_users::delete_expired`, hourly, so a trial user is gone
    /// within the hour after their time is up.
    TrialUsersDelete,
    /// `avatars::delete_unreferenced`.
    AvatarsDeleteUnreferenced,
    /// `files::delete_unreferenced`.
    FilesDeleteUnreferenced,
    /// `accounts::stripe_reconcile::run`, on a server with Stripe: what a
    /// missed webhook left behind.
    StripeReconcile,
    /// `accounts::app_store_reconcile::run`, on a server with the App
    /// Store: what a missed notification left behind.
    AppStoreReconcile,
}

impl Job {
    /// Every job, in the order `run_due` runs them.
    pub const ALL: [Job; 6] = [
        Job::Purge,
        Job::TrialUsersDelete,
        Job::AvatarsDeleteUnreferenced,
        Job::FilesDeleteUnreferenced,
        Job::StripeReconcile,
        Job::AppStoreReconcile,
    ];

    /// Its name in `app_jobs` and on the command line.
    pub fn name(self) -> &'static str {
        match self {
            Job::Purge => "purge",
            Job::TrialUsersDelete => "trial_users_delete",
            Job::AvatarsDeleteUnreferenced => "avatars_delete_unreferenced",
            Job::FilesDeleteUnreferenced => "files_delete_unreferenced",
            Job::StripeReconcile => "stripe_reconcile",
            Job::AppStoreReconcile => "app_store_reconcile",
        }
    }

    /// The job with this name, if there is one.
    pub fn from_name(name: &str) -> Option<Job> {
        Job::ALL.into_iter().find(|job| job.name() == name)
    }
}

/// A job's last run, as `app_jobs` keeps it.
#[derive(Clone, Debug, PartialEq)]
pub struct JobRecord {
    pub job: Job,
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
pub async fn get(tx: &mut Transaction, job: Job) -> Result<Option<JobRecord>, sqlx::Error> {
    query!(tx, get(job))
}

/// Explicit processing operations; bulk SQL and cascades do not run per-row hooks.
pub mod processing {
    use super::*;

    /// Atomically claims a due job; false if another caller already claimed it.
    pub async fn claim_if_due(
        tx: &mut Transaction,
        job: Job,
        now: DateTime<Utc>,
        due_after: DateTime<Utc>,
    ) -> Result<bool, sqlx::Error> {
        query!(tx, claim_if_due(job, now, due_after))
    }

    /// Records a forced job start in the caller transaction.
    pub async fn claim(
        tx: &mut Transaction,
        job: Job,
        now: DateTime<Utc>,
    ) -> Result<(), sqlx::Error> {
        query!(tx, claim(job, now))
    }

    /// Stores a job completion report or failure in the caller transaction.
    pub async fn finish(
        tx: &mut Transaction,
        job: Job,
        finished_at: DateTime<Utc>,
        last_report: Option<&Value>,
        last_error: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        query!(tx, finish(job, finished_at, last_report, last_error))
    }
}
