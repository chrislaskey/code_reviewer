//! The memberships commands: joining and leaving a community, for the
//! user signed in on this device. Both start a sync, which backfills a
//! community just joined and prunes one just left.

use tauri::{AppHandle, State};
use uuid::Uuid;

use app_core::app::memberships;
use app_core::data::app_users_communities::Membership;

use crate::current_user::signed_in;
use crate::device_accounts::DeviceAccounts;
use crate::sync;

#[tauri::command]
pub async fn memberships_join(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    community_id: Uuid,
) -> Result<Membership, String> {
    let (deployment, user) = signed_in(&device).await?;
    let membership = memberships::join(&deployment, &user, community_id)
        .await
        .map_err(|error| error.to_string())?;
    sync::spawn(app);
    Ok(membership)
}

#[tauri::command]
pub async fn memberships_leave(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    community_id: Uuid,
) -> Result<(), String> {
    let (deployment, user) = signed_in(&device).await?;
    memberships::leave(&deployment, &user, community_id)
        .await
        .map_err(|error| error.to_string())?;
    sync::spawn(app);
    Ok(())
}
