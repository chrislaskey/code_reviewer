//! Where to PUT the picture the person picked, asked of the web server,
//! which holds the bucket's keys.

use tauri::State;

use app_core::app::avatars::uploads;
use app_core::app::avatars::{AvatarUpload, AvatarUploadNew};

use crate::command_error::CommandError;
use crate::device_accounts::DeviceAccounts;
use crate::sync::SyncServer;

#[tauri::command]
pub async fn avatars_uploads_create(
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
    content_type: String,
    size: u64,
) -> Result<AvatarUpload, CommandError> {
    let deployment = device.active().await?;
    let request = AvatarUploadNew { content_type, size };
    Ok(uploads::create_on_device(&deployment, server.transport(), &request).await?)
}
