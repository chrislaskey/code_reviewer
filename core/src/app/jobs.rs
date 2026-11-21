//! Recurring jobs: what runs, on which role, how often and in what order,
//! decided here once for every app, see `docs/design/jobs.md`. An app
//! only wakes them up, as often as it can, by calling `run_due`: a job
//! not yet due does nothing, and one missed runs at the next call.
//!
//! Each run is recorded in `app_jobs`, one row per job. Starting a run
//! is a conditional write, so of several servers calling at once only
//! one runs it. A job that fails is recorded with its error and tried
//! again once it is due; every job may be run again safely.

use crate::data::app_jobs::{self, Job, JobRecord};
use chrono::{DateTime, Duration, Utc};
use serde_json::Value;
use thiserror::Error;

use crate::app::avatars::AvatarError;
use crate::app::files::FileError;
use crate::deployment::{Deployment, Role};
use crate::sync::purge::{self, PurgeError};

impl Job {
    /// The role of the deployments it runs on.
    pub fn role(self) -> Role {
        match self {
            Job::Purge
            | Job::TrialUsersDelete
            | Job::AvatarsDeleteUnreferenced
            | Job::FilesDeleteUnreferenced
            | Job::StripeReconcile
            | Job::AppStoreReconcile => Role::Server,
        }
    }

    /// How long after a run starts the next is due.
    pub fn interval(self) -> Duration {
        match self {
            Job::Purge
            | Job::AvatarsDeleteUnreferenced
            | Job::FilesDeleteUnreferenced
            | Job::StripeReconcile
            | Job::AppStoreReconcile => Duration::days(1),
            Job::TrialUsersDelete => Duration::hours(1),
        }
    }

    /// Whether the deployment has what the job works on: a sweep needs
    /// its bucket.
    fn is_set_up(self, deployment: &Deployment) -> bool {
        match self {
            Job::Purge | Job::TrialUsersDelete => true,
            #[cfg(feature = "object-storage")]
            Job::AvatarsDeleteUnreferenced => deployment.avatar_bucket.is_some(),
            #[cfg(feature = "object-storage")]
            Job::FilesDeleteUnreferenced => matches!(
                deployment.file_store,
                crate::deployment::FileStore::Bucket(_)
            ),
            #[cfg(not(feature = "object-storage"))]
            Job::AvatarsDeleteUnreferenced | Job::FilesDeleteUnreferenced => {
                let _ = deployment;
                false
            }
            #[cfg(feature = "stripe")]
            Job::StripeReconcile => deployment.stripe.is_some(),
            #[cfg(not(feature = "stripe"))]
            Job::StripeReconcile => false,
            #[cfg(feature = "app-store")]
            Job::AppStoreReconcile => deployment.app_store.is_some(),
            #[cfg(not(feature = "app-store"))]
            Job::AppStoreReconcile => false,
        }
    }
}

/// One run by `run_due`: the job's report, or its error.
#[derive(Debug)]
pub struct JobRun {
    pub job: Job,
    pub result: Result<Value, JobError>,
}

#[derive(Debug, Error)]
pub enum JobError {
    #[error("the {} job does not run on this deployment", .0.name())]
    NotForThisRole(Job),
    #[error("the {} job is not set up here: no bucket, no Stripe, or no App Store", .0.name())]
    NotSetUp(Job),
    #[error(transparent)]
    Purge(#[from] PurgeError),
    #[error(transparent)]
    TrialUser(#[from] crate::accounts::trial_users::TrialUserError),
    #[error(transparent)]
    Avatar(#[from] AvatarError),
    #[error(transparent)]
    File(#[from] FileError),
    #[cfg(feature = "stripe")]
    #[error(transparent)]
    StripeReconcile(#[from] crate::accounts::stripe_reconcile::StripeReconcileError),
    #[cfg(feature = "app-store")]
    #[error(transparent)]
    AppStoreReconcile(#[from] crate::accounts::app_store_reconcile::AppStoreReconcileError),
    #[error("could not record the report: {0}")]
    Report(#[from] serde_json::Error),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// Runs, in order, every job of this deployment's role that is set up
/// here and due at `now`, and records each run. One failing does not
/// stop the next; only failing to record a run stops them all.
pub async fn run_due(deployment: &Deployment, now: DateTime<Utc>) -> Result<Vec<JobRun>, JobError> {
    let mut runs = Vec::new();
    for job in Job::ALL {
        if job.role() != deployment.role || !job.is_set_up(deployment) {
            continue;
        }
        let mut tx = deployment.begin().await?;
        let claimed =
            app_jobs::processing::claim_if_due(&mut tx, job, now, now - job.interval()).await?;
        tx.commit().await?;
        if !claimed {
            continue;
        }
        let result = perform(deployment, job, now).await;
        finish(deployment, job, &result).await?;
        runs.push(JobRun { job, result });
    }
    Ok(runs)
}

/// Runs the job at `now`, due or not, and records the run.
pub async fn run(deployment: &Deployment, job: Job, now: DateTime<Utc>) -> Result<Value, JobError> {
    if job.role() != deployment.role {
        return Err(JobError::NotForThisRole(job));
    }
    if !job.is_set_up(deployment) {
        return Err(JobError::NotSetUp(job));
    }
    let mut tx = deployment.begin().await?;
    app_jobs::processing::claim(&mut tx, job, now).await?;
    tx.commit().await?;
    let result = perform(deployment, job, now).await;
    finish(deployment, job, &result).await?;
    result
}

/// The job's last run, or none before its first.
pub async fn get(deployment: &Deployment, job: Job) -> Result<Option<JobRecord>, JobError> {
    let mut tx = deployment.begin().await?;
    let record = app_jobs::get(&mut tx, job).await?;
    tx.commit().await?;
    Ok(record)
}

async fn perform(deployment: &Deployment, job: Job, now: DateTime<Utc>) -> Result<Value, JobError> {
    match job {
        Job::Purge => Ok(serde_json::to_value(purge::run(deployment, now).await?)?),
        Job::TrialUsersDelete => Ok(serde_json::to_value(
            crate::accounts::trial_users::delete_expired(deployment, now).await?,
        )?),
        #[cfg(feature = "object-storage")]
        Job::AvatarsDeleteUnreferenced => Ok(serde_json::to_value(
            crate::app::avatars::delete_unreferenced(deployment, now).await?,
        )?),
        #[cfg(feature = "object-storage")]
        Job::FilesDeleteUnreferenced => Ok(serde_json::to_value(
            crate::app::files::delete_unreferenced(deployment, now).await?,
        )?),
        #[cfg(not(feature = "object-storage"))]
        Job::AvatarsDeleteUnreferenced | Job::FilesDeleteUnreferenced => {
            Err(JobError::NotSetUp(job))
        }
        #[cfg(feature = "stripe")]
        Job::StripeReconcile => match &deployment.stripe {
            Some(stripe) => Ok(serde_json::to_value(
                crate::accounts::stripe_reconcile::run(deployment, stripe).await?,
            )?),
            None => Err(JobError::NotSetUp(job)),
        },
        #[cfg(not(feature = "stripe"))]
        Job::StripeReconcile => Err(JobError::NotSetUp(job)),
        #[cfg(feature = "app-store")]
        Job::AppStoreReconcile => match &deployment.app_store {
            Some(app_store) => Ok(serde_json::to_value(
                crate::accounts::app_store_reconcile::run(deployment, app_store).await?,
            )?),
            None => Err(JobError::NotSetUp(job)),
        },
        #[cfg(not(feature = "app-store"))]
        Job::AppStoreReconcile => Err(JobError::NotSetUp(job)),
    }
}

/// Records the end of the run, on this deployment's clock.
async fn finish(
    deployment: &Deployment,
    job: Job,
    result: &Result<Value, JobError>,
) -> Result<(), JobError> {
    let (report, error) = match result {
        Ok(report) => (Some(report), None),
        Err(error) => (None, Some(error.to_string())),
    };
    let finished_at = crate::deployment::now();
    let mut tx = deployment.begin().await?;
    app_jobs::processing::finish(&mut tx, job, finished_at, report, error.as_deref()).await?;
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::{Database, with_each_database};

    fn server(database: Database) -> Deployment {
        Deployment::new(database, Role::Server)
    }

    fn jobs_of(runs: &[JobRun]) -> Vec<Job> {
        runs.iter().map(|run| run.job).collect()
    }

    #[tokio::test]
    async fn jobs_rolled_back_claim_can_be_reclaimed_and_committed_claim_is_exclusive() {
        with_each_database(|database| async move {
            let deployment = server(database);
            let now = crate::deployment::now();
            let due_after = now - Duration::days(1);
            let mut tx = deployment.begin().await.expect("transaction");
            assert!(
                app_jobs::processing::claim_if_due(&mut tx, Job::Purge, now, due_after)
                    .await
                    .expect("claim")
            );
            drop(tx);
            assert!(get(&deployment, Job::Purge).await.expect("read").is_none());
            let mut tx = deployment.begin().await.expect("transaction");
            assert!(
                app_jobs::processing::claim_if_due(&mut tx, Job::Purge, now, due_after)
                    .await
                    .expect("reclaim")
            );
            tx.commit().await.expect("commit");
            let mut tx = deployment.begin().await.expect("transaction");
            assert!(
                !app_jobs::processing::claim_if_due(&mut tx, Job::Purge, now, due_after)
                    .await
                    .expect("already claimed")
            );
            tx.commit().await.expect("commit");
        })
        .await;
    }

    #[tokio::test]
    async fn jobs_run_due_runs_the_purge_once_a_day_and_the_trial_delete_hourly_and_records_them() {
        with_each_database(|database| async move {
            let deployment = server(database);
            let now = crate::deployment::now();

            let runs = run_due(&deployment, now).await.expect("the jobs");
            assert_eq!(
                jobs_of(&runs),
                [Job::Purge, Job::TrialUsersDelete],
                "no buckets, no sweeps"
            );
            assert!(runs[0].result.is_ok());
            let record = get(&deployment, Job::Purge)
                .await
                .expect("a record")
                .expect("the purge's");
            assert_eq!(record.started_at, now);
            assert!(record.finished_at.is_some());
            assert_eq!(record.last_error, None);
            assert!(record.last_report.expect("a report")["horizon"].is_string());

            let almost_an_hour = now + Duration::minutes(59);
            let runs = run_due(&deployment, almost_an_hour)
                .await
                .expect("the jobs");
            assert!(runs.is_empty(), "not due yet");

            let almost_a_day = now + Duration::hours(23);
            let runs = run_due(&deployment, almost_a_day).await.expect("the jobs");
            assert_eq!(jobs_of(&runs), [Job::TrialUsersDelete], "the hourly one");
            assert_eq!(
                runs[0].result.as_ref().expect("a report")["users"],
                serde_json::json!(0)
            );

            let a_day_later = now + Duration::days(1);
            let runs = run_due(&deployment, a_day_later).await.expect("the jobs");
            assert_eq!(jobs_of(&runs), [Job::Purge, Job::TrialUsersDelete]);
        })
        .await;
    }

    #[tokio::test]
    async fn jobs_claim_if_due_lets_one_of_two_servers_at_once() {
        with_each_database(|database| async move {
            let deployment = server(database);
            let now = crate::deployment::now();
            let a_day_ago = now - Duration::days(1);
            let mut tx = deployment.begin().await.expect("a transaction");
            let first =
                app_jobs::processing::claim_if_due(&mut tx, Job::Purge, now, a_day_ago).await;
            let second =
                app_jobs::processing::claim_if_due(&mut tx, Job::Purge, now, a_day_ago).await;
            tx.commit().await.expect("committed");
            assert!(first.expect("a claim"));
            assert!(!second.expect("a claim"), "the first one runs it");
        })
        .await;
    }

    #[tokio::test]
    async fn jobs_run_runs_a_job_not_yet_due() {
        with_each_database(|database| async move {
            let deployment = server(database);
            let now = crate::deployment::now();
            run_due(&deployment, now).await.expect("the jobs");

            let later = now + Duration::minutes(5);
            run(&deployment, Job::Purge, later)
                .await
                .expect("the purge");
            let record = get(&deployment, Job::Purge)
                .await
                .expect("a record")
                .expect("the purge's");
            assert_eq!(record.started_at, later);
        })
        .await;
    }

    #[tokio::test]
    async fn jobs_run_refuses_a_job_of_another_role_or_without_its_bucket() {
        let device = crate::app::users::tests::device().await;
        let error = run(&device, Job::Purge, crate::deployment::now())
            .await
            .expect_err("a device");
        assert!(matches!(error, JobError::NotForThisRole(Job::Purge)));
        let runs = run_due(&device, crate::deployment::now())
            .await
            .expect("the jobs");
        assert!(runs.is_empty(), "a device has no jobs yet");

        let database = Database::open_sqlite_in_memory().await.expect("a database");
        let error = run(
            &server(database),
            Job::FilesDeleteUnreferenced,
            crate::deployment::now(),
        )
        .await
        .expect_err("no bucket");
        assert!(matches!(
            error,
            JobError::NotSetUp(Job::FilesDeleteUnreferenced)
        ));
    }

    #[test]
    fn jobs_from_name_knows_every_job() {
        for job in Job::ALL {
            assert_eq!(Job::from_name(job.name()), Some(job));
        }
        assert_eq!(Job::from_name("nothing"), None);
    }

    #[cfg(feature = "object-storage")]
    mod with_buckets {
        use super::*;
        use crate::deployment::FileStore;
        use crate::services::storage::Storage;

        /// A server with everything its jobs work on: buckets, and
        /// Stripe when it sells.
        fn server_with_buckets(database: Database) -> Deployment {
            let deployment = server(database)
                .with_avatar_bucket(Storage::memory())
                .with_file_store(FileStore::Bucket(Storage::memory()));
            #[cfg(feature = "stripe")]
            let deployment = deployment.with_stripe(crate::services::stripe::Stripe::memory());
            #[cfg(feature = "app-store")]
            let deployment =
                deployment.with_app_store(crate::services::app_store::AppStore::memory());
            deployment
        }

        #[tokio::test]
        async fn jobs_run_due_runs_the_purge_and_then_the_sweeps() {
            with_each_database(|database| async move {
                let deployment = server_with_buckets(database);
                let runs = run_due(&deployment, crate::deployment::now())
                    .await
                    .expect("the jobs");
                assert_eq!(jobs_of(&runs), Job::ALL);
                assert!(runs.iter().all(|run| run.result.is_ok()), "{runs:?}");
            })
            .await;
        }

        #[tokio::test]
        async fn jobs_run_due_records_an_error_and_runs_the_next() {
            with_each_database(|database| async move {
                let deployment = server_with_buckets(database);
                drop_purges_table(&deployment).await;

                let runs = run_due(&deployment, crate::deployment::now())
                    .await
                    .expect("the jobs");
                assert_eq!(jobs_of(&runs), Job::ALL);
                assert!(runs[0].result.is_err(), "the purge has no table");
                assert!(runs[1].result.is_ok() && runs[2].result.is_ok());
                let record = get(&deployment, Job::Purge)
                    .await
                    .expect("a record")
                    .expect("the purge's");
                assert!(record.last_error.is_some());
                assert_eq!(record.last_report, None);
            })
            .await;
        }

        async fn drop_purges_table(deployment: &Deployment) {
            let statement = "DROP TABLE app_purges";
            match &deployment.database {
                Database::Sqlite(pool) => {
                    sqlx::query(statement).execute(pool).await.expect("dropped");
                }
                #[cfg(feature = "postgres")]
                Database::Postgres(pool) => {
                    sqlx::query(statement).execute(pool).await.expect("dropped");
                }
            }
        }
    }
}
