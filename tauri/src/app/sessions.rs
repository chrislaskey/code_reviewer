//! The sessions commands: each asks the server's API as the signed-in
//! person, see `app_core::app::sessions::online`, and answers its JSON as
//! it came. The sessions run on the server's machine.

use serde_json::Value;
use tauri::{AppHandle, State};

use app_core::app::sessions::online;

use crate::command_error::CommandError;
use crate::device_accounts::DeviceAccounts;
use crate::sync;

#[tauri::command]
pub async fn sessions_settings_get(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
) -> Result<Value, CommandError> {
    let (deployment, transport) = device.active_with_transport().await?;
    online::settings_get(&deployment, &transport)
        .await
        .map_err(|error| sync::online_error(&app, error))
}

#[tauri::command]
pub async fn sessions_default_port_get(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
) -> Result<Value, CommandError> {
    let (deployment, transport) = device.active_with_transport().await?;
    online::default_port_get(&deployment, &transport)
        .await
        .map_err(|error| sync::online_error(&app, error))
}

#[tauri::command]
pub async fn sessions_list(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
) -> Result<Value, CommandError> {
    let (deployment, transport) = device.active_with_transport().await?;
    online::list(&deployment, &transport)
        .await
        .map_err(|error| sync::online_error(&app, error))
}

#[tauri::command]
pub async fn sessions_create(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    request: Value,
) -> Result<Value, CommandError> {
    let (deployment, transport) = device.active_with_transport().await?;
    online::create(&deployment, &transport, &request)
        .await
        .map_err(|error| sync::online_error(&app, error))
}

#[tauri::command]
pub async fn sessions_get(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    name: String,
) -> Result<Value, CommandError> {
    let (deployment, transport) = device.active_with_transport().await?;
    online::get(&deployment, &transport, &name)
        .await
        .map_err(|error| sync::online_error(&app, error))
}

#[tauri::command]
pub async fn sessions_delete(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    name: String,
) -> Result<Value, CommandError> {
    let (deployment, transport) = device.active_with_transport().await?;
    online::delete(&deployment, &transport, &name)
        .await
        .map_err(|error| sync::online_error(&app, error))
}

#[tauri::command]
pub async fn sessions_chat_get(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    name: String,
) -> Result<Value, CommandError> {
    let (deployment, transport) = device.active_with_transport().await?;
    online::chat_get(&deployment, &transport, &name)
        .await
        .map_err(|error| sync::online_error(&app, error))
}

#[tauri::command]
pub async fn sessions_prompts_create(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    name: String,
    request: Value,
) -> Result<Value, CommandError> {
    let (deployment, transport) = device.active_with_transport().await?;
    online::prompts_create(&deployment, &transport, &name, &request)
        .await
        .map_err(|error| sync::online_error(&app, error))
}

#[tauri::command]
pub async fn sessions_prompt_images_create(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    name: String,
    request: Value,
) -> Result<Value, CommandError> {
    let (deployment, transport) = device.active_with_transport().await?;
    online::prompt_images_create(&deployment, &transport, &name, &request)
        .await
        .map_err(|error| sync::online_error(&app, error))
}

#[tauri::command]
pub async fn sessions_continuations_create(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    name: String,
) -> Result<Value, CommandError> {
    let (deployment, transport) = device.active_with_transport().await?;
    online::continuations_create(&deployment, &transport, &name)
        .await
        .map_err(|error| sync::online_error(&app, error))
}

#[tauri::command]
pub async fn sessions_merges_create(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    name: String,
    request: Value,
) -> Result<Value, CommandError> {
    let (deployment, transport) = device.active_with_transport().await?;
    online::merges_create(&deployment, &transport, &name, &request)
        .await
        .map_err(|error| sync::online_error(&app, error))
}

#[tauri::command]
pub async fn sessions_terminal_windows_create(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    name: String,
) -> Result<Value, CommandError> {
    let (deployment, transport) = device.active_with_transport().await?;
    online::terminal_windows_create(&deployment, &transport, &name)
        .await
        .map_err(|error| sync::online_error(&app, error))
}

#[tauri::command]
pub async fn sessions_browser_url_get(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    name: String,
) -> Result<Value, CommandError> {
    let (deployment, transport) = device.active_with_transport().await?;
    online::browser_url_get(&deployment, &transport, &name)
        .await
        .map_err(|error| sync::online_error(&app, error))
}

#[tauri::command]
pub async fn sessions_artifacts_url_get(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    name: String,
) -> Result<Value, CommandError> {
    let (deployment, transport) = device.active_with_transport().await?;
    online::artifacts_url_get(&deployment, &transport, &name)
        .await
        .map_err(|error| sync::online_error(&app, error))
}
