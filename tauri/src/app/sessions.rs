//! The sessions commands: each asks the server's API as the signed-in
//! person, see `app_core::app::sessions::online`, and answers its JSON as
//! it came. The sessions run on the server's machine.

use serde_json::Value;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

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

/// Opens a session's `http://localhost:<port>` in the system browser.
#[tauri::command]
pub async fn sessions_browser_open(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    name: String,
) -> Result<(), CommandError> {
    let (deployment, transport) = device.active_with_transport().await?;
    let url = online::browser_url_get(&deployment, &transport, &name)
        .await
        .map_err(|error| sync::online_error(&app, error))?;
    let url = url.as_str().ok_or("the server answered no address")?;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|error| format!("could not open the browser: {error}"))?;
    Ok(())
}

/// Opens a session's artifacts in the system browser.
#[tauri::command]
pub async fn sessions_artifacts_open(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    name: String,
) -> Result<(), CommandError> {
    let (deployment, transport) = device.active_with_transport().await?;
    let url = online::artifacts_url_get(&deployment, &transport, &name)
        .await
        .map_err(|error| sync::online_error(&app, error))?;
    let url = url.as_str().ok_or("the server answered no address")?;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|error| format!("could not open the browser: {error}"))?;
    Ok(())
}

#[tauri::command]
pub async fn sessions_bell_get(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    name: String,
) -> Result<Value, CommandError> {
    let (deployment, transport) = device.active_with_transport().await?;
    online::bell_get(&deployment, &transport, &name)
        .await
        .map_err(|error| sync::online_error(&app, error))
}

/// On a phone, turning a bell on first gives the server the push token,
/// asking the person's permission the first time.
#[tauri::command]
pub async fn sessions_bell_update(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    name: String,
    enabled: bool,
) -> Result<Value, CommandError> {
    #[cfg(target_os = "ios")]
    if enabled {
        crate::app::apple_push_registration::register_for_apple_push(&app).await?;
    }
    let (deployment, transport) = device.active_with_transport().await?;
    online::bell_update(&deployment, &transport, &name, enabled)
        .await
        .map_err(|error| sync::online_error(&app, error))
}
