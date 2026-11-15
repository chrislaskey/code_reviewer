//! The `app_jobs` table: each recurring job's last run.

use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::PgConnection;
use sqlx::types::Json;

use crate::jobs::{Job, JobRecord};

/// Starts the job's run at `now` unless one started after `due_after`;
/// whether it did, so of two servers asking at once only one runs it.
pub async fn claim_if_due(
    connection: &mut PgConnection,
    job: Job,
    now: DateTime<Utc>,
    due_after: DateTime<Utc>,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO app_jobs (name, started_at) VALUES ($1, $2)
         ON CONFLICT (name) DO UPDATE SET
           started_at = excluded.started_at, finished_at = NULL
         WHERE app_jobs.started_at <= $3",
    )
    .bind(job.name())
    .bind(now)
    .bind(due_after)
    .execute(connection)
    .await?;
    Ok(result.rows_affected() == 1)
}

/// Starts the job's run at `now`, due or not.
pub async fn claim(
    connection: &mut PgConnection,
    job: Job,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO app_jobs (name, started_at) VALUES ($1, $2)
         ON CONFLICT (name) DO UPDATE SET
           started_at = excluded.started_at, finished_at = NULL",
    )
    .bind(job.name())
    .bind(now)
    .execute(connection)
    .await?;
    Ok(())
}

/// Records the end of the job's run: its report, or its error.
pub async fn finish(
    connection: &mut PgConnection,
    job: Job,
    finished_at: DateTime<Utc>,
    last_report: Option<&Value>,
    last_error: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE app_jobs SET finished_at = $2, last_report = $3, last_error = $4
         WHERE name = $1",
    )
    .bind(job.name())
    .bind(finished_at)
    .bind(last_report.map(Json))
    .bind(last_error)
    .execute(connection)
    .await?;
    Ok(())
}

pub async fn get(
    connection: &mut PgConnection,
    job: Job,
) -> Result<Option<JobRecord>, sqlx::Error> {
    type Row = (
        DateTime<Utc>,
        Option<DateTime<Utc>>,
        Option<Json<Value>>,
        Option<String>,
    );
    let row: Option<Row> = sqlx::query_as(
        "SELECT started_at, finished_at, last_report, last_error FROM app_jobs WHERE name = $1",
    )
    .bind(job.name())
    .fetch_optional(connection)
    .await?;
    Ok(row.map(
        |(started_at, finished_at, last_report, last_error)| JobRecord {
            job,
            started_at,
            finished_at,
            last_report: last_report.map(|Json(report)| report),
            last_error,
        },
    ))
}
