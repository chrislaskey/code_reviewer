//! The code that confirms deleting the signed-in person's account,
//! mailed by the server.

use tauri::{AppHandle, State};

use app_core::accounts::online;

use crate::command_error::CommandError;
use crate::device_accounts::DeviceAccounts;
use crate::sync::SyncServer;

/// Has the server mail the code that confirms the delete.
#[tauri::command]
pub async fn accounts_deletions_code_create(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
) -> Result<(), CommandError> {
    let deployment = device.active().await?;
    online::deletions_code_create(&deployment, server.transport())
        .await
        .map_err(|error| crate::sync::online_error(&app, error))
}
