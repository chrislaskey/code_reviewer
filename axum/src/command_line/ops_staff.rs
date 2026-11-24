//! `app_web ops staff ...`: who may use the ops app, said from the
//! server's command line and never from a page, see
//! `docs/design/multi-app.md`.

use app_core::deployment::Deployment;
use app_core::ops::staff;

/// `create <email>` and `delete <email>`.
pub async fn run(deployment: &Deployment, words: &[&str]) -> Result<(), String> {
    match words {
        ["create", email] => {
            staff::create(deployment, email)
                .await
                .map_err(|error| error.to_string())?;
            println!("{email} is staff");
            Ok(())
        }
        ["delete", email] => {
            staff::delete(deployment, email)
                .await
                .map_err(|error| error.to_string())?;
            println!("{email} is staff no longer");
            Ok(())
        }
        _ => Err("commands: ops staff create <email>, ops staff delete <email>".to_string()),
    }
}
