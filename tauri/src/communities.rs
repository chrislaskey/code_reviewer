//! The communities commands: `app_core::app::communities` as the pages call
//! it, for the user signed in on this device. A write starts a sync. A
//! form's error is `{error, errors}`, as the web's 422 body is.

use tauri::{AppHandle, State};
use uuid::Uuid;

use app_core::app::communities::{self, Community, CommunityAndMembership, CommunityError};

use crate::avatars::FormError;
use crate::current_user::signed_in;
use crate::device_accounts::DeviceAccounts;
use crate::sync;

impl From<CommunityError> for FormError {
    fn from(error: CommunityError) -> FormError {
        match error {
            CommunityError::Invalid(errors) => FormError::of_fields(errors),
            other => FormError::from(other.to_string()),
        }
    }
}

#[tauri::command]
pub async fn communities_list(
    device: State<'_, DeviceAccounts>,
) -> Result<Vec<CommunityAndMembership>, String> {
    let (deployment, user) = signed_in(&device).await?;
    communities::list(&deployment, &user)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn communities_get(
    device: State<'_, DeviceAccounts>,
    id: Uuid,
) -> Result<CommunityAndMembership, String> {
    let (deployment, user) = signed_in(&device).await?;
    communities::get(&deployment, &user, id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn communities_create(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    name: String,
) -> Result<Community, FormError> {
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
) -> Result<Community, FormError> {
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
) -> Result<(), String> {
    let (deployment, user) = signed_in(&device).await?;
    communities::delete(&deployment, &user, id)
        .await
        .map_err(|error| error.to_string())?;
    sync::spawn(app);
    Ok(())
}
