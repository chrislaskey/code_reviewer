//! The pending changes command: `app_core::app::pending_changes` as the
//! page listing the changes not on the server calls it.

use tauri::State;

use app_core::app::pending_changes::{self, PendingChange};

use crate::device_accounts::DeviceAccounts;

#[tauri::command]
pub async fn pending_changes_list(
    device: State<'_, DeviceAccounts>,
) -> Result<Vec<PendingChange>, String> {
    let deployment = device.active().await.map_err(|error| error.to_string())?;
    pending_changes::list(&deployment)
        .await
        .map_err(|error| error.to_string())
}
