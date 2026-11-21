//! The server's recurring jobs, see `app_core::jobs`: woken up by a
//! loop every `TICK` while the server runs, and by `app_web jobs run`
//! for a scheduler outside it, see `command_line::jobs`. Both run only
//! what is due, so they never run a job twice in its interval.

use std::time::Duration;

use app_core::deployment::{self, Deployment};
use app_core::jobs;

/// How often the loop asks for the jobs due.
const TICK: Duration = Duration::from_secs(15 * 60);

/// Runs the jobs due now, at startup and then every `TICK`.
pub fn spawn(deployment: Deployment) {
    tokio::spawn(async move {
        loop {
            match jobs::run_due(&deployment, deployment::now()).await {
                Ok(runs) => {
                    for run in runs {
                        match run.result {
                            Ok(report) => tracing::info!("job {} ran: {report}", run.job.name()),
                            Err(error) => tracing::warn!("job {} failed: {error}", run.job.name()),
                        }
                    }
                }
                Err(error) => tracing::warn!("could not run the jobs: {error}"),
            }
            tokio::time::sleep(TICK).await;
        }
    });
}
