//! The signed-in user's preferences: the language the pages are shown
//! in, written on this device and synced when it next can, see
//! `app_core::accounts::preferences`.

use serde_json::Value;
use tauri::{AppHandle, State};

use app_core::accounts::preferences::{self, Preferences};

use crate::command_error::CommandError;
use crate::device_accounts::DeviceAccounts;
use crate::sync;
use crate::user_auth::signed_in;

/// The signed-in user's preferences.
#[tauri::command]
pub async fn accounts_preferences_get(
    device: State<'_, DeviceAccounts>,
) -> Result<Preferences, CommandError> {
    let (deployment, user) = signed_in(&device).await?;
    Ok(preferences::get(&deployment, &user).await?)
}

/// One preference of the signed-in user's, null for the default, then
/// all of them.
#[tauri::command]
pub async fn accounts_preferences_update(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    key: String,
    value: Value,
) -> Result<Preferences, CommandError> {
    let (deployment, user) = signed_in(&device).await?;
    let updated = preferences::update(&deployment, &user, &key, value).await?;
    sync::spawn(app);
    Ok(updated)
}
