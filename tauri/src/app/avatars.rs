//! The avatars commands: `app_core::app::avatars` as the pages call it with
//! `invoke`, for the user signed in on this device. Online only: both
//! ask the web server, which holds the bucket's keys; the page PUTs the
//! file to the URL the first answers. An error is `{error, errors}`, the
//! web's 422 body, so a page shows the fields' messages either way.

pub mod uploads;

use tauri::State;

use app_core::app::avatars::{self, AvatarUpdate};
use app_core::data::app_users::User;

use crate::command_error::CommandError;
use crate::device_accounts::DeviceAccounts;
use crate::sync::SyncServer;

#[tauri::command]
pub async fn avatars_update(
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
    key: String,
) -> Result<User, CommandError> {
    let deployment = device.active().await?;
    let request = AvatarUpdate { key };
    Ok(avatars::update_on_device(&deployment, server.transport(), &request).await?)
}
