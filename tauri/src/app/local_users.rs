//! The local user's commands, see `app_core::app::local_users` and
//! `docs/design/accounts-local-and-trial-users.md`: Skip on the accounts
//! page makes a user that is only on this device and opens the app as
//! them, with no request made; when they later sign up or sign in, the
//! person chooses whether what they made moves into the account, which
//! then syncs, or stays apart beside it.

use tauri::{AppHandle, State};

use app_core::data::app_users::User;

use crate::command_error::CommandError;
use crate::device_accounts::DeviceAccounts;
use crate::sync::{self, SyncServer};

/// Whether the accounts page shows Skip.
#[tauri::command]
pub async fn local_users_can_create(
    device: State<'_, DeviceAccounts>,
) -> Result<bool, CommandError> {
    Ok(device.local_user_can_create().await?)
}

/// Skip: the device's local user, made and active.
#[tauri::command]
pub async fn local_users_create(
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
) -> Result<User, CommandError> {
    let _no_sync_meanwhile = server.hold().await;
    Ok(device.local_user_create(&device_name()).await?)
}

/// Moves what the local user made into the account whose sign-in
/// offered it, and syncs.
#[tauri::command]
pub async fn local_users_move(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
) -> Result<User, CommandError> {
    let user = {
        let _no_sync_meanwhile = server.hold().await;
        device.local_user_move().await?
    };
    sync::spawn(app);
    Ok(user)
}

/// Adds the account whose sign-in offered a move beside the local
/// user, which stays as it was, and syncs.
#[tauri::command]
pub async fn local_users_keep_separate(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
) -> Result<User, CommandError> {
    let user = {
        let _no_sync_meanwhile = server.hold().await;
        device.local_user_keep_separate().await?
    };
    sync::spawn(app);
    Ok(user)
}

/// What the system calls this device, "Alice's MacBook Pro", or nothing
/// when it will not say; a phone answers little more than its model.
#[cfg(not(target_os = "ios"))]
fn device_name() -> String {
    whoami::devicename().unwrap_or_default()
}

/// Nothing on iOS: `whoami` asks SystemConfiguration there, which has the
/// call only on macOS, so the app would not link.
#[cfg(target_os = "ios")]
fn device_name() -> String {
    String::new()
}
