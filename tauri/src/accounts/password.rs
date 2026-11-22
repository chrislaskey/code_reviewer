//! The password command: the signed-in user's password, kept by the
//! server, so only when online.

use tauri::State;

use app_core::accounts::password::{self, PasswordUpdate};

use crate::command_error::CommandError;
use crate::device_accounts::DeviceAccounts;
use crate::sync::SyncServer;

/// The signed-in user's password, their first or a new one, kept by
/// the server.
#[tauri::command]
pub async fn accounts_password_update(
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
    current: Option<String>,
    password: String,
) -> Result<(), CommandError> {
    let deployment = device.active().await?;
    let update = PasswordUpdate { current, password };
    Ok(password::update_on_device(&deployment, server.transport(), &update).await?)
}
