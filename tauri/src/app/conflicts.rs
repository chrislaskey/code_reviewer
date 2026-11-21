//! The conflicts commands: `app_core::app::conflicts` as the conflicts page
//! calls it. A change reapplied or tried again starts a sync, see `sync`.

use tauri::{AppHandle, State};
use uuid::Uuid;

use app_core::app::conflicts::{self, Conflict};

use crate::command_error::CommandError;
use crate::device_accounts::DeviceAccounts;
use crate::sync;
use crate::user_auth::signed_in;

#[tauri::command]
pub async fn conflicts_list(
    device: State<'_, DeviceAccounts>,
) -> Result<Vec<Conflict>, CommandError> {
    let deployment = device.active().await?;
    Ok(conflicts::list(&deployment).await?)
}

#[tauri::command]
pub async fn conflicts_reapply(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    id: Uuid,
) -> Result<(), CommandError> {
    let (deployment, user) = signed_in(&device).await?;
    conflicts::reapply(&deployment, &user, id).await?;
    sync::spawn(app);
    Ok(())
}

#[tauri::command]
pub async fn conflicts_try_again(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    id: Uuid,
) -> Result<(), CommandError> {
    let deployment = device.active().await?;
    conflicts::try_again(&deployment, id).await?;
    sync::spawn(app);
    Ok(())
}

#[tauri::command]
pub async fn conflicts_let_go(
    device: State<'_, DeviceAccounts>,
    id: Uuid,
) -> Result<(), CommandError> {
    let deployment = device.active().await?;
    Ok(conflicts::let_go(&deployment, id).await?)
}
