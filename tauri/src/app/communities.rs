//! The communities commands: `app_core::app::communities` as the pages call
//! it, for the user signed in on this device. A write starts a sync. A
//! form's error is `{error, errors}`, as the web's 422 body is.

use tauri::{AppHandle, State};
use uuid::Uuid;

use app_core::app::communities::{self, CommunityAndMembership};
use app_core::data::app_communities::Community;

use crate::command_error::CommandError;
use crate::device_accounts::DeviceAccounts;
use crate::sync;
use crate::user_auth::signed_in;

#[tauri::command]
pub async fn communities_list(
    device: State<'_, DeviceAccounts>,
) -> Result<Vec<CommunityAndMembership>, CommandError> {
    let (deployment, user) = signed_in(&device).await?;
    Ok(communities::list(&deployment, &user).await?)
}

#[tauri::command]
pub async fn communities_get(
    device: State<'_, DeviceAccounts>,
    id: Uuid,
) -> Result<CommunityAndMembership, CommandError> {
    let (deployment, user) = signed_in(&device).await?;
    Ok(communities::get(&deployment, &user, id).await?)
}

#[tauri::command]
pub async fn communities_create(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    name: String,
) -> Result<Community, CommandError> {
    let (deployment, user) = signed_in(&device).await?;
    let community = communities::create(&deployment, &user, &name).await?;
    sync::spawn(app);
    Ok(community)
}

#[tauri::command]
pub async fn communities_update(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    id: Uuid,
    name: String,
) -> Result<Community, CommandError> {
    let (deployment, user) = signed_in(&device).await?;
    let community = communities::update(&deployment, &user, id, &name).await?;
    sync::spawn(app);
    Ok(community)
}

#[tauri::command]
pub async fn communities_delete(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    id: Uuid,
) -> Result<(), CommandError> {
    let (deployment, user) = signed_in(&device).await?;
    communities::delete(&deployment, &user, id).await?;
    sync::spawn(app);
    Ok(())
}
