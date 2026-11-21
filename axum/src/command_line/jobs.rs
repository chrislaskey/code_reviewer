//! `app_web jobs ...`: the recurring jobs run from the command line, for
//! a scheduler outside the server, see `app_core::app::jobs`.

use app_core::app::jobs;
use app_core::data::app_jobs::Job;
use app_core::deployment::{self, Deployment};

/// `jobs run`, the jobs due now, or `jobs run <name>`, that job now,
/// printing what each did.
pub async fn run(deployment: &Deployment, words: &[&str]) -> Result<(), String> {
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
