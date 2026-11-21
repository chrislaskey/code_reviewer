//! The conflicts commands: `app_core::app::conflicts` as the conflicts page
//! calls it. A change reapplied or tried again starts a sync, see `sync`.

use tauri::{AppHandle, State};
use uuid::Uuid;

use app_core::app::conflicts::{self, Conflict};

use crate::device_accounts::DeviceAccounts;
use crate::sync;
use crate::user_auth::signed_in;

#[tauri::command]
pub async fn conflicts_list(device: State<'_, DeviceAccounts>) -> Result<Vec<Conflict>, String> {
    let deployment = device.active().await.map_err(|error| error.to_string())?;
    conflicts::list(&deployment)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn conflicts_reapply(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    id: Uuid,
) -> Result<(), String> {
    let (deployment, user) = signed_in(&device).await?;
    conflicts::reapply(&deployment, &user, id)
        .await
        .map_err(|error| error.to_string())?;
    sync::spawn(app);
    Ok(())
}

#[tauri::command]
pub async fn conflicts_try_again(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    id: Uuid,
) -> Result<(), String> {
    let deployment = device.active().await.map_err(|error| error.to_string())?;
    conflicts::try_again(&deployment, id)
        .await
        .map_err(|error| error.to_string())?;
    sync::spawn(app);
    Ok(())
}

#[tauri::command]
pub async fn conflicts_let_go(device: State<'_, DeviceAccounts>, id: Uuid) -> Result<(), String> {
    let deployment = device.active().await.map_err(|error| error.to_string())?;
    conflicts::let_go(&deployment, id)
        .await
        .map_err(|error| error.to_string())
}
