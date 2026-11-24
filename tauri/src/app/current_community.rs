//! The current community commands: the community the person works in on
//! this device, kept as a device setting of the signed-in account, see
//! `app_core::app::current_community`. Never synced, so no sync starts.

use tauri::State;
use uuid::Uuid;

use app_core::app::current_community;
use app_core::data::app_communities::Community;

use crate::command_error::CommandError;
use crate::device_accounts::DeviceAccounts;
use crate::user_auth::signed_in;

#[tauri::command]
pub async fn current_community_get(
    device: State<'_, DeviceAccounts>,
) -> Result<Option<Community>, CommandError> {
    let (deployment, user) = signed_in(&device).await?;
    Ok(current_community::get_on_device(&deployment, &user).await?)
}

#[tauri::command]
pub async fn current_community_update(
    device: State<'_, DeviceAccounts>,
    community_id: Uuid,
) -> Result<Community, CommandError> {
    let (deployment, user) = signed_in(&device).await?;
    Ok(current_community::update_on_device(&deployment, &user, community_id).await?)
}
