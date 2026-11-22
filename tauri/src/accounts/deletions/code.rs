//! The code that confirms deleting the signed-in person's account,
//! mailed by the server.

use tauri::{AppHandle, State};

use app_core::accounts::online;

use crate::command_error::CommandError;
use crate::device_accounts::DeviceAccounts;

/// Has the server mail the code that confirms the delete.
#[tauri::command]
pub async fn accounts_deletions_code_create(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
) -> Result<(), CommandError> {
    let (deployment, transport) = device.active_with_transport().await?;
    online::deletions_code_create(&deployment, &transport)
        .await
        .map_err(|error| crate::sync::online_error(&app, error))
}
