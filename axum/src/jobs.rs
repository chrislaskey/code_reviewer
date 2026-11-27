//! The server's recurring jobs, see `app_core::jobs`: woken up by a
//! loop every `TICK` while the server runs, and by `app_web jobs run`
//! for a scheduler outside it, see `command_line::jobs`. Both run only
//! what is due, so they never run a job twice in its interval. Beside a
//! remote backend only the jobs on what Rust keeps run, due by when this
//! process last ran them, see `jobs::run_due_beside_a_remote_backend`.

use std::collections::BTreeMap;
use std::time::Duration;

use app_core::deployment::{self, Deployment};
use app_core::jobs;

/// How often the loop asks for the jobs due.
const TICK: Duration = Duration::from_secs(15 * 60);

/// Runs the jobs due now, at startup and then every `TICK`.
pub fn spawn(deployment: Deployment) {
    tokio::spawn(async move {
        let mut last_runs = BTreeMap::new();
        loop {
            let now = deployment::now();
            let runs = if deployment.has_remote_backend() {
                Ok(jobs::run_due_beside_a_remote_backend(&deployment, now, &mut last_runs).await)
            } else {
                jobs::run_due(&deployment, now).await
            };
            match runs {
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
