//! The password command: the signed-in user's password, kept by the
//! server, so only when online.

use tauri::State;

use app_core::accounts::password::{self, PasswordUpdate};

use crate::command_error::CommandError;
use crate::device_accounts::DeviceAccounts;

/// The signed-in user's password, their first or a new one, kept by
/// the server.
#[tauri::command]
pub async fn accounts_password_update(
    device: State<'_, DeviceAccounts>,
    current: Option<String>,
    password: String,
) -> Result<(), CommandError> {
    let (deployment, transport) = device.active_with_transport().await?;
    let update = PasswordUpdate { current, password };
    Ok(password::update_on_device(&deployment, &transport, &update).await?)
}
