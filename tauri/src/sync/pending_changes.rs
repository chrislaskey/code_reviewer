//! The pending changes command: `app_core::sync::pending_changes` as the
//! page listing the changes not on the server calls it.

use tauri::State;

use app_core::sync::pending_changes::{self, PendingChange};

use crate::command_error::CommandError;
use crate::device_accounts::DeviceAccounts;

#[tauri::command]
pub async fn pending_changes_list(
    device: State<'_, DeviceAccounts>,
) -> Result<Vec<PendingChange>, CommandError> {
    let deployment = device.active().await?;
    Ok(pending_changes::list(&deployment).await?)
}
