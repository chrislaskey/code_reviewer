//! The user signed in on the active account, for every command that acts
//! as them, with the account's deployment, see `device_accounts`; an
//! error, "Nobody is signed in", until someone is active.

use app_core::app::users;
use app_core::data::app_users::User;
use app_core::deployment::Deployment;
use app_core::services::web_server::Http;

use crate::command_error::CommandError;
use crate::device_accounts::DeviceAccounts;

/// The active account's deployment and its signed-in user.
pub async fn signed_in(device: &DeviceAccounts) -> Result<(Deployment, User), CommandError> {
    let deployment = device.active().await?;
    let user = users::signed_in(&deployment).await?;
    Ok((deployment, user))
}

/// `signed_in` with the transport to the account's server, for a command
/// that also asks the server, read together with the deployment.
pub async fn signed_in_with_transport(
    device: &DeviceAccounts,
) -> Result<(Deployment, Http, User), CommandError> {
    let (deployment, transport) = device.active_with_transport().await?;
    let user = users::signed_in(&deployment).await?;
    Ok((deployment, transport, user))
}
