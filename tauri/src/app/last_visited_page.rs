//! The last visited page command: the router keeps the page the person
//! is on as a device setting of the active account, see
//! `app_core::app::last_visited_page`. A switch to the account reads it
//! back, see `device_accounts`. Never synced, so no sync starts.

use tauri::State;

use app_core::app::last_visited_page;

use crate::command_error::CommandError;
use crate::device_accounts::DeviceAccounts;

#[tauri::command]
pub async fn last_visited_page_update(
    device: State<'_, DeviceAccounts>,
    path: String,
) -> Result<(), CommandError> {
    let deployment = device.active().await?;
    Ok(last_visited_page::update_on_device(&deployment, &path).await?)
}
