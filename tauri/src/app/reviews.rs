//! The reviews commands: each asks the server's API as the signed-in
//! person, see `app_core::app::reviews::online`, and answers its JSON as
//! it came. The repository is on the server's machine.

use serde_json::Value;
use tauri::{AppHandle, State};

use app_core::app::reviews::online;

use crate::command_error::CommandError;
use crate::device_accounts::DeviceAccounts;
use crate::sync;

#[tauri::command]
pub async fn reviews_branches_list(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    repo_path: String,
) -> Result<Value, CommandError> {
    let (deployment, transport) = device.active_with_transport().await?;
    online::branches_list(&deployment, &transport, &repo_path)
        .await
        .map_err(|error| sync::online_error(&app, error))
}

#[tauri::command]
pub async fn reviews_file_changes_list(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    request: Value,
) -> Result<Value, CommandError> {
    let (deployment, transport) = device.active_with_transport().await?;
    online::file_changes_list(&deployment, &transport, &request)
        .await
        .map_err(|error| sync::online_error(&app, error))
}

#[tauri::command]
pub async fn reviews_function_diffs_list(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    request: Value,
) -> Result<Value, CommandError> {
    let (deployment, transport) = device.active_with_transport().await?;
    online::function_diffs_list(&deployment, &transport, &request)
        .await
        .map_err(|error| sync::online_error(&app, error))
}

#[tauri::command]
pub async fn reviews_file_diffs_list(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    request: Value,
) -> Result<Value, CommandError> {
    let (deployment, transport) = device.active_with_transport().await?;
    online::file_diffs_list(&deployment, &transport, &request)
        .await
        .map_err(|error| sync::online_error(&app, error))
}
