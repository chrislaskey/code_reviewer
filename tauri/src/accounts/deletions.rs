//! Deleting the signed-in person's account from the app, see
//! `docs/design/accounts-delete.md`: the server mails a code and deletes
//! the account with it, `app_core::accounts::online`, and the device
//! then removes the account and its file, as removing it from the
//! device's list does. A local user has no account to delete. Another
//! device of the same person learns from the server's 410, see `sync`.

pub mod code;

use tauri::{AppHandle, State};

use app_core::accounts::online;

use crate::command_error::CommandError;
use crate::device_accounts::DeviceAccounts;
use crate::sync::SyncServer;

/// Deletes the account with the mailed code, then removes it and its
/// file from the device; nobody is active after.
#[tauri::command]
pub async fn accounts_deletions_create(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
    code: String,
) -> Result<(), CommandError> {
    let (deployment, transport) = device.active_with_transport().await?;
    let id = device.active_id().await.ok_or("Nobody is signed in")?;
    online::deletions_create(&deployment, &transport, &code)
        .await
        .map_err(|error| crate::sync::online_error(&app, error))?;
    let _no_sync_meanwhile = server.hold().await;
    device.remove(&transport, id, true).await?;
    Ok(())
}
