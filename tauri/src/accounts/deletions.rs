//! Deleting the signed-in person's account from the app, see
//! `docs/design/accounts-delete.md`: the server mails a code and deletes
//! the account with it, `app_core::accounts::online`, and the device
//! then removes the account and its file, as removing it from the
//! device's list does. A local user has no account to delete. Another
//! device of the same person learns from the server's 410, see `sync`.

use tauri::{AppHandle, State};

use app_core::accounts::online;

use crate::device_accounts::DeviceAccounts;
use crate::sync::SyncServer;

/// Has the server mail the code that confirms the delete.
#[tauri::command]
pub async fn accounts_deletions_code_create(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
) -> Result<(), String> {
    let deployment = device.active().await.map_err(|error| error.to_string())?;
    online::deletions_code_create(&deployment, server.transport())
        .await
        .map_err(|error| crate::sync::online_error(&app, error))
}

/// Deletes the account with the mailed code, then removes it and its
/// file from the device; nobody is active after.
#[tauri::command]
pub async fn accounts_deletions_create(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
    code: String,
) -> Result<(), String> {
    let deployment = device.active().await.map_err(|error| error.to_string())?;
    let user_id = device.active_user_id().await.ok_or("Nobody is signed in")?;
    online::deletions_create(&deployment, server.transport(), &code)
        .await
        .map_err(|error| crate::sync::online_error(&app, error))?;
    let _no_sync_meanwhile = server.hold().await;
    device
        .remove(server.transport(), user_id, true)
        .await
        .map_err(|error| error.to_string())?;
    Ok(())
}
