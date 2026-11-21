//! Recurring jobs: what runs, on which role, how often and in what order,
//! see `docs/design/jobs.md`. Each is a `RecurringJob`: the template's
//! are here, an app's own in `app::jobs`, run after them. An app only
//! wakes them up, as often as it can, by calling `run_due`: a job not
//! yet due does nothing, and one missed runs at the next call.
//!
//! Each run is recorded in `app_jobs`, one row per job, by its name.
//! Starting a run is a conditional write, so of several servers calling
//! at once only one runs it. A job that fails is recorded with its error
//! and tried again once it is due; every job may be run again safely.

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use serde_json::Value;
use thiserror::Error;

use crate::app::avatars::AvatarError;
use crate::data::app_jobs::{self, JobRecord};
use crate::deployment::{Deployment, Role};
use crate::sync::bytes::BytesError;
use crate::sync::purge::{self, PurgeError};

/// The error of an app's own job, as the runner records it.
pub type JobFailure = Box<dyn std::error::Error + Send + Sync>;

/// A job that runs again and again, each run recorded under its name.
#[async_trait]
pub trait RecurringJob: Send + Sync + std::fmt::Debug {
    /// Its name in `app_jobs` and on the command line.
    fn name(&self) -> &'static str;

    /// The role of the deployments it runs on.
    fn role(&self) -> Role;

    /// How long after a run starts the next is due.
    fn interval(&self) -> Duration;

    /// Whether the deployment has what the job works on: a sweep needs
    /// its bucket.
    fn is_set_up(&self, _deployment: &Deployment) -> bool {
        true
    }

    /// Runs it once, answering what it did.
    async fn perform(&self, deployment: &Deployment, now: DateTime<Utc>)
    -> Result<Value, JobError>;
}

/// `purge::run`, before the sweeps, so the bytes of the rows it removes
/// go in the same day.
#[derive(Debug)]
pub struct Purge;

#[async_trait]
impl RecurringJob for Purge {
    fn name(&self) -> &'static str {
        "purge"
    }

    fn role(&self) -> Role {
        Role::Server
    }

    fn interval(&self) -> Duration {
        Duration::days(1)
    }

    async fn perform(
        &self,
        deployment: &Deployment,
        now: DateTime<Utc>,
    ) -> Result<Value, JobError> {
        Ok(serde_json::to_value(purge::run(deployment, now).await?)?)
    }
}

/// `trial_users::delete_expired`, hourly, so a trial user is gone within
/// the hour after their time is up.
#[derive(Debug)]
pub struct TrialUsersDelete;

#[async_trait]
impl RecurringJob for TrialUsersDelete {
    fn name(&self) -> &'static str {
        "trial_users_delete"
    }

    fn role(&self) -> Role {
        Role::Server
    }

    fn interval(&self) -> Duration {
        Duration::hours(1)
    }

    async fn perform(
        &self,
        deployment: &Deployment,
        now: DateTime<Utc>,
    ) -> Result<Value, JobError> {
        Ok(serde_json::to_value(
            crate::accounts::trial_users::delete_expired(deployment, now).await?,
        )?)
    }
}

/// `avatars::delete_unreferenced`, on a server with a bucket for them.
#[derive(Debug)]
pub struct AvatarsDeleteUnreferenced;

#[async_trait]
impl RecurringJob for AvatarsDeleteUnreferenced {
    fn name(&self) -> &'static str {
        "avatars_delete_unreferenced"
    }

    fn role(&self) -> Role {
        Role::Server
    }

    fn interval(&self) -> Duration {
        Duration::days(1)
    }

    #[cfg(feature = "object-storage")]
    fn is_set_up(&self, deployment: &Deployment) -> bool {
        deployment.avatar_bucket.is_some()
    }

    #[cfg(not(feature = "object-storage"))]
    fn is_set_up(&self, _deployment: &Deployment) -> bool {
        false
    }

    #[cfg(feature = "object-storage")]
    async fn perform(
        &self,
        deployment: &Deployment,
        now: DateTime<Utc>,
    ) -> Result<Value, JobError> {
        Ok(serde_json::to_value(
            crate::app::avatars::delete_unreferenced(deployment, now).await?,
        )?)
    }

    #[cfg(not(feature = "object-storage"))]
    async fn perform(
        &self,
        _deployment: &Deployment,
        _now: DateTime<Utc>,
    ) -> Result<Value, JobError> {
        Err(JobError::NotSetUp(self.name()))
    }
}

/// `bytes::delete_unreferenced`, the bytes of every synced table that
/// has them, on a server with a bucket for them.
#[derive(Debug)]
pub struct FilesDeleteUnreferenced;

#[async_trait]
impl RecurringJob for FilesDeleteUnreferenced {
    fn name(&self) -> &'static str {
        "files_delete_unreferenced"
    }

    fn role(&self) -> Role {
        Role::Server
    }

    fn interval(&self) -> Duration {
        Duration::days(1)
    }

    #[cfg(feature = "object-storage")]
    fn is_set_up(&self, deployment: &Deployment) -> bool {
        matches!(
            deployment.file_store,
            crate::deployment::FileStore::Bucket(_)
        )
    }

    #[cfg(not(feature = "object-storage"))]
    fn is_set_up(&self, _deployment: &Deployment) -> bool {
        false
    }

    #[cfg(feature = "object-storage")]
    async fn perform(
        &self,
        deployment: &Deployment,
        now: DateTime<Utc>,
    ) -> Result<Value, JobError> {
        Ok(serde_json::to_value(
            crate::sync::bytes::delete_unreferenced(deployment, now).await?,
        )?)
    }

    #[cfg(not(feature = "object-storage"))]
    async fn perform(
        &self,
        _deployment: &Deployment,
        _now: DateTime<Utc>,
    ) -> Result<Value, JobError> {
        Err(JobError::NotSetUp(self.name()))
    }
}

/// `accounts::stripe_reconcile::run`, on a server with Stripe: what a
/// missed webhook left behind.
#[derive(Debug)]
pub struct StripeReconcile;

#[async_trait]
impl RecurringJob for StripeReconcile {
    fn name(&self) -> &'static str {
        "stripe_reconcile"
    }

    fn role(&self) -> Role {
        Role::Server
    }

    fn interval(&self) -> Duration {
        Duration::days(1)
    }

    #[cfg(feature = "stripe")]
    fn is_set_up(&self, deployment: &Deployment) -> bool {
        deployment.stripe.is_some()
    }

    #[cfg(not(feature = "stripe"))]
    fn is_set_up(&self, _deployment: &Deployment) -> bool {
        false
    }

    #[cfg(feature = "stripe")]
    async fn perform(
        &self,
        deployment: &Deployment,
        _now: DateTime<Utc>,
    ) -> Result<Value, JobError> {
        let stripe = deployment
            .stripe
            .as_ref()
            .ok_or(JobError::NotSetUp(self.name()))?;
        Ok(serde_json::to_value(
            crate::accounts::stripe_reconcile::run(deployment, stripe).await?,
        )?)
    }

    #[cfg(not(feature = "stripe"))]
    async fn perform(
        &self,
        _deployment: &Deployment,
        _now: DateTime<Utc>,
    ) -> Result<Value, JobError> {
        Err(JobError::NotSetUp(self.name()))
    }
}

/// `accounts::app_store_reconcile::run`, on a server with the App Store:
/// what a missed notification left behind.
#[derive(Debug)]
pub struct AppStoreReconcile;

#[async_trait]
impl RecurringJob for AppStoreReconcile {
    fn name(&self) -> &'static str {
        "app_store_reconcile"
    }

    fn role(&self) -> Role {
        Role::Server
    }

    fn interval(&self) -> Duration {
        Duration::days(1)
    }

    #[cfg(feature = "app-store")]
    fn is_set_up(&self, deployment: &Deployment) -> bool {
        deployment.app_store.is_some()
    }

    #[cfg(not(feature = "app-store"))]
    fn is_set_up(&self, _deployment: &Deployment) -> bool {
        false
    }

    #[cfg(feature = "app-store")]
    async fn perform(
        &self,
        deployment: &Deployment,
        _now: DateTime<Utc>,
    ) -> Result<Value, JobError> {
        let app_store = deployment
            .app_store
            .as_ref()
            .ok_or(JobError::NotSetUp(self.name()))?;
        Ok(serde_json::to_value(
            crate::accounts::app_store_reconcile::run(deployment, app_store).await?,
        )?)
    }

    #[cfg(not(feature = "app-store"))]
    async fn perform(
        &self,
        _deployment: &Deployment,
        _now: DateTime<Utc>,
    ) -> Result<Value, JobError> {
        Err(JobError::NotSetUp(self.name()))
    }
}

/// The template's jobs, in the order `run_due` runs them.
const TEMPLATE: [&dyn RecurringJob; 6] = [
    &Purge,
    &TrialUsersDelete,
    &AvatarsDeleteUnreferenced,
    &FilesDeleteUnreferenced,
    &StripeReconcile,
    &AppStoreReconcile,
];

/// Every job, the template's and then the app's, in the order `run_due`
/// runs them.
pub fn list() -> Vec<&'static dyn RecurringJob> {
    TEMPLATE
        .into_iter()
        .chain(crate::app::jobs::list().iter().copied())
        .collect()
}

/// The job with this name, if there is one.
pub fn get(name: &str) -> Option<&'static dyn RecurringJob> {
    list().into_iter().find(|job| job.name() == name)
}

/// One run by `run_due`: the job's report, or its error.
#[derive(Debug)]
pub struct JobRun {
    pub job: &'static dyn RecurringJob,
    pub result: Result<Value, JobError>,
}

#[derive(Debug, Error)]
pub enum JobError {
    #[error("the {0} job does not run on this deployment")]
    NotForThisRole(&'static str),
    #[error("the {0} job is not set up here: no bucket, no Stripe, or no App Store")]
    NotSetUp(&'static str),
    #[error(transparent)]
    Purge(#[from] PurgeError),
    #[error(transparent)]
    TrialUser(#[from] crate::accounts::trial_users::TrialUserError),
    #[error(transparent)]
    Avatar(#[from] AvatarError),
    #[error(transparent)]
    Bytes(#[from] BytesError),
    #[cfg(feature = "stripe")]
    #[error(transparent)]
    StripeReconcile(#[from] crate::accounts::stripe_reconcile::StripeReconcileError),
    #[cfg(feature = "app-store")]
    #[error(transparent)]
    AppStoreReconcile(#[from] crate::accounts::app_store_reconcile::AppStoreReconcileError),
    /// An app's own job failed.
    #[error(transparent)]
    Failed(#[from] JobFailure),
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
    for job in list() {
        if job.role() != deployment.role || !job.is_set_up(deployment) {
            continue;
        }
        let mut tx = deployment.begin().await?;
        let claimed =
            app_jobs::processing::claim_if_due(&mut tx, job.name(), now, now - job.interval())
                .await?;
        tx.commit().await?;
        if !claimed {
            continue;
        }
        let result = job.perform(deployment, now).await;
        finish(deployment, job, &result).await?;
        runs.push(JobRun { job, result });
    }
    Ok(runs)
}

/// Runs the job at `now`, due or not, and records the run.
pub async fn run(
    deployment: &Deployment,
    job: &dyn RecurringJob,
    now: DateTime<Utc>,
) -> Result<Value, JobError> {
    if job.role() != deployment.role {
        return Err(JobError::NotForThisRole(job.name()));
    }
    if !job.is_set_up(deployment) {
        return Err(JobError::NotSetUp(job.name()));
    }
    let mut tx = deployment.begin().await?;
    app_jobs::processing::claim(&mut tx, job.name(), now).await?;
    tx.commit().await?;
    let result = job.perform(deployment, now).await;
    finish(deployment, job, &result).await?;
    result
}

/// The job's last run, or none before its first.
pub async fn get_last_run(
    deployment: &Deployment,
    job: &dyn RecurringJob,
) -> Result<Option<JobRecord>, JobError> {
    let mut tx = deployment.begin().await?;
    let record = app_jobs::get(&mut tx, job.name()).await?;
    tx.commit().await?;
    Ok(record)
}

/// Records the end of the run, on this deployment's clock.
async fn finish(
    deployment: &Deployment,
    job: &dyn RecurringJob,
    result: &Result<Value, JobError>,
) -> Result<(), JobError> {
    let (report, error) = match result {
        Ok(report) => (Some(report), None),
        Err(error) => (None, Some(error.to_string())),
    };
    let finished_at = crate::deployment::now();
    let mut tx = deployment.begin().await?;
    app_jobs::processing::finish(&mut tx, job.name(), finished_at, report, error.as_deref())
        .await?;
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

    fn jobs_of(runs: &[JobRun]) -> Vec<&'static str> {
        runs.iter().map(|run| run.job.name()).collect()
    }

    #[tokio::test]
    async fn jobs_rolled_back_claim_can_be_reclaimed_and_committed_claim_is_exclusive() {
        with_each_database(|database| async move {
            let deployment = server(database);
            let now = crate::deployment::now();
            let due_after = now - Duration::days(1);
            let mut tx = deployment.begin().await.expect("transaction");
            assert!(
                app_jobs::processing::claim_if_due(&mut tx, Purge.name(), now, due_after)
                    .await
                    .expect("claim")
            );
            drop(tx);
            assert!(
                get_last_run(&deployment, &Purge)
                    .await
                    .expect("read")
                    .is_none()
            );
            let mut tx = deployment.begin().await.expect("transaction");
            assert!(
                app_jobs::processing::claim_if_due(&mut tx, Purge.name(), now, due_after)
                    .await
                    .expect("reclaim")
            );
            tx.commit().await.expect("commit");
            let mut tx = deployment.begin().await.expect("transaction");
            assert!(
                !app_jobs::processing::claim_if_due(&mut tx, Purge.name(), now, due_after)
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
                ["purge", "trial_users_delete"],
                "no buckets, no sweeps"
            );
            assert!(runs[0].result.is_ok());
            let record = get_last_run(&deployment, &Purge)
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
            assert_eq!(jobs_of(&runs), ["trial_users_delete"], "the hourly one");
            assert_eq!(
                runs[0].result.as_ref().expect("a report")["users"],
                serde_json::json!(0)
            );

            let a_day_later = now + Duration::days(1);
            let runs = run_due(&deployment, a_day_later).await.expect("the jobs");
            assert_eq!(jobs_of(&runs), ["purge", "trial_users_delete"]);
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
                app_jobs::processing::claim_if_due(&mut tx, Purge.name(), now, a_day_ago).await;
            let second =
                app_jobs::processing::claim_if_due(&mut tx, Purge.name(), now, a_day_ago).await;
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
            run(&deployment, &Purge, later).await.expect("the purge");
            let record = get_last_run(&deployment, &Purge)
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
        let error = run(&device, &Purge, crate::deployment::now())
            .await
            .expect_err("a device");
        assert!(matches!(error, JobError::NotForThisRole("purge")));
        let runs = run_due(&device, crate::deployment::now())
            .await
            .expect("the jobs");
        assert!(runs.is_empty(), "a device has no jobs yet");

        let database = Database::open_sqlite_in_memory().await.expect("a database");
        let error = run(
            &server(database),
            &FilesDeleteUnreferenced,
            crate::deployment::now(),
        )
        .await
        .expect_err("no bucket");
        assert!(matches!(
            error,
            JobError::NotSetUp("files_delete_unreferenced")
        ));
    }

    #[test]
    fn jobs_get_knows_every_job_by_its_name() {
        for job in list() {
            assert_eq!(get(job.name()).map(|found| found.name()), Some(job.name()));
        }
        assert!(get("nothing").is_none());
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
                assert_eq!(
                    jobs_of(&runs),
                    list().iter().map(|job| job.name()).collect::<Vec<_>>()
                );
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
                assert_eq!(
                    jobs_of(&runs),
                    list().iter().map(|job| job.name()).collect::<Vec<_>>()
                );
                assert!(runs[0].result.is_err(), "the purge has no table");
                assert!(runs[1].result.is_ok() && runs[2].result.is_ok());
                let record = get_last_run(&deployment, &Purge)
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
