//! The purchases commands: the signed-in person's one-time purchases.
//! Each asks the server's API as the signed-in person, see
//! `app_core::accounts::online`, and answers its JSON as it came.

use serde_json::Value;
use tauri::{AppHandle, State};
use uuid::Uuid;

use app_core::accounts::online;

use crate::command_error::CommandError;
use crate::device_accounts::DeviceAccounts;
use crate::sync;

#[tauri::command]
pub async fn accounts_purchases_list(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
) -> Result<Value, CommandError> {
    let (deployment, transport) = device.active_with_transport().await?;
    online::purchases_list(&deployment, &transport)
        .await
        .map_err(|error| sync::online_error(&app, error))
}

#[tauri::command]
pub async fn accounts_purchases_get(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    id: Uuid,
) -> Result<Value, CommandError> {
    let (deployment, transport) = device.active_with_transport().await?;
    online::purchases_get(&deployment, &transport, id)
        .await
        .map_err(|error| sync::online_error(&app, error))
}
