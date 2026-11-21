//! The purchases commands: the signed-in person's one-time purchases.
//! Each asks the server's API as the signed-in person, see
//! `app_core::accounts::online`, and answers its JSON as it came.

use serde_json::Value;
use tauri::{AppHandle, State};
use uuid::Uuid;

use app_core::accounts::online;

use crate::device_accounts::DeviceAccounts;
use crate::sync::{self, SyncServer};

#[tauri::command]
pub async fn accounts_purchases_list(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
) -> Result<Value, String> {
    let deployment = device.active().await.map_err(|error| error.to_string())?;
    online::purchases_list(&deployment, server.transport())
        .await
        .map_err(|error| sync::online_error(&app, error))
}

#[tauri::command]
pub async fn accounts_purchases_get(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
    id: Uuid,
) -> Result<Value, String> {
    let deployment = device.active().await.map_err(|error| error.to_string())?;
    online::purchases_get(&deployment, server.transport(), id)
        .await
        .map_err(|error| sync::online_error(&app, error))
}
