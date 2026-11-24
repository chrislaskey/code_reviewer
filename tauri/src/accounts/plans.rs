//! The person's plan as the device keeps it, for the pages offline, see
//! `app_core::accounts::online`.

use app_core::accounts::online;
use app_core::deployment::Deployment;
use app_core::services::web_server::Http;

/// Asks for the plan again, after a change; a failure leaves the plan
/// kept as it was, for the next sync to try.
pub async fn refresh(deployment: &Deployment, server: &Http) {
    if let Err(error) = online::plan_refresh(deployment, server).await {
        log::warn!("could not refresh the plan: {error}");
    }
}
