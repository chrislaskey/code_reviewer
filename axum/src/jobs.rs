//! The server's recurring jobs, see `app_core::app::jobs`: woken up by a loop
//! every `TICK` while the server runs, and by `app_web jobs run` for a
//! scheduler outside it. Both run only what is due, so they never run a
//! job twice in its interval.

use std::time::Duration;

use app_core::app::jobs;
use app_core::data::app_jobs::Job;
use app_core::deployment::{self, Deployment};

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

/// `jobs run`, the jobs due now, or `jobs run <name>`, that job now,
/// printing what each did.
pub async fn command(deployment: &Deployment, words: &[&str]) -> Result<(), String> {
    let now = deployment::now();
    match words {
        ["run"] => {
            let runs = jobs::run_due(deployment, now)
                .await
                .map_err(|error| error.to_string())?;
            if runs.is_empty() {
                println!("no job is due");
            }
            let mut failed = false;
            for run in runs {
                match run.result {
                    Ok(report) => println!("{} ran: {report}", run.job.name()),
                    Err(error) => {
                        failed = true;
                        println!("{} failed: {error}", run.job.name());
                    }
                }
            }
            if failed {
                return Err("a job failed".to_string());
            }
            Ok(())
        }
        ["run", name] => {
            let job = Job::from_name(name).ok_or_else(usage)?;
            let report = jobs::run(deployment, job, now)
                .await
                .map_err(|error| error.to_string())?;
            println!("{} ran: {report}", job.name());
            Ok(())
        }
        _ => Err(usage()),
    }
}

fn usage() -> String {
    let names: Vec<&str> = Job::ALL.into_iter().map(Job::name).collect();
    format!("commands: jobs run, jobs run <{}>", names.join("|"))
}
