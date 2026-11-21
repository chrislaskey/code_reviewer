//! The current community commands: the community the person works in on
//! this device, kept as a device setting of the signed-in account, see
//! `app_core::app::current_community`. Never synced, so no sync starts.

use tauri::State;
use uuid::Uuid;

use app_core::app::communities::Community;
use app_core::app::current_community;

use crate::current_user::signed_in;
use crate::device_accounts::DeviceAccounts;

#[tauri::command]
pub async fn current_community_get(
    device: State<'_, DeviceAccounts>,
) -> Result<Option<Community>, String> {
    let (deployment, user) = signed_in(&device).await?;
    current_community::get_on_device(&deployment, &user)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn current_community_update(
    device: State<'_, DeviceAccounts>,
    community_id: Uuid,
) -> Result<Community, String> {
    let (deployment, user) = signed_in(&device).await?;
    current_community::update_on_device(&deployment, &user, community_id)
        .await
        .map_err(|error| error.to_string())
}
