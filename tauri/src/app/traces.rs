//! The traces commands: each asks the server's API as the signed-in
//! person, see `app_core::app::traces::online`, and answers its JSON as
//! it came. The repository is on the server's machine.

use serde_json::Value;
use tauri::{AppHandle, State};

use app_core::app::traces::online;

use crate::command_error::CommandError;
use crate::device_accounts::DeviceAccounts;
use crate::sync;

#[tauri::command]
pub async fn traces_tests_list(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    request: Value,
) -> Result<Value, CommandError> {
    let (deployment, transport) = device.active_with_transport().await?;
    online::tests_list(&deployment, &transport, &request)
        .await
        .map_err(|error| sync::online_error(&app, error))
}

#[tauri::command]
pub async fn traces_call_stacks_get(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    request: Value,
) -> Result<Value, CommandError> {
    let (deployment, transport) = device.active_with_transport().await?;
    online::call_stacks_get(&deployment, &transport, &request)
        .await
        .map_err(|error| sync::online_error(&app, error))
}
