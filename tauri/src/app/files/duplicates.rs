//! The files the user sees on this device with the same bytes as those
//! picked, see `app_core::app::files::duplicates`.

use tauri::State;

use app_core::app::files::FileWithState;
use app_core::app::files::duplicates;

use crate::command_error::CommandError;
use crate::device_accounts::DeviceAccounts;
use crate::user_auth::signed_in;

/// The files the user sees on this device with any of these checksums.
#[tauri::command]
pub async fn files_duplicates_list(
    device: State<'_, DeviceAccounts>,
    sha256s: Vec<String>,
) -> Result<Vec<FileWithState>, CommandError> {
    let (deployment, user) = signed_in(&device).await?;
    Ok(duplicates::list(&deployment, &user, &sha256s).await?)
}
