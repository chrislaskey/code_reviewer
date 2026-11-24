//! The password commands: whether the signed-in user has a password,
//! and their first or a new one, kept by the server, so only when
//! online.

use tauri::State;

use app_core::accounts::password::{self, PasswordStatus, PasswordUpdate};

use crate::command_error::CommandError;
use crate::device_accounts::DeviceAccounts;

/// Whether the signed-in user has a password, asked of the server.
#[tauri::command]
pub async fn accounts_password_get(
    device: State<'_, DeviceAccounts>,
) -> Result<PasswordStatus, CommandError> {
    let (deployment, transport) = device.active_with_transport().await?;
    Ok(password::get_on_device(&deployment, &transport).await?)
}

/// The signed-in user's password, their first or a new one, kept by
/// the server.
#[tauri::command]
pub async fn accounts_password_update(
    device: State<'_, DeviceAccounts>,
    password: String,
) -> Result<(), CommandError> {
    let (deployment, transport) = device.active_with_transport().await?;
    let update = PasswordUpdate { password };
    Ok(password::update_on_device(&deployment, &transport, &update).await?)
}
