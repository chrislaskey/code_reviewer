//! `app_web <command>`: a command on the database, run instead of the
//! server, one module per command.

pub mod accounts_products;
pub mod jobs;
pub mod ops_staff;

use app_core::deployment::Deployment;
use app_core::services::stripe::Stripe;

/// Runs the command the arguments name.
pub async fn run(
    deployment: &Deployment,
    stripe: Option<&Stripe>,
    arguments: &[String],
) -> Result<(), String> {
    let words: Vec<&str> = arguments.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["jobs", rest @ ..] => jobs::run(deployment, rest).await,
        ["ops", "staff", rest @ ..] => ops_staff::run(deployment, rest).await,
        ["accounts", "products", rest @ ..] => accounts_products::run(deployment, stripe, rest).await,
        _ => Err(
            "commands: jobs run, jobs run <name>, ops staff create <email>, ops staff delete <email>, \
             accounts products sync"
                .to_string(),
        ),
    }
}
