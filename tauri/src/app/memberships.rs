//! The memberships commands: joining and leaving a community, for the
//! user signed in on this device. Both start a sync, which backfills a
//! community just joined and prunes one just left.

use tauri::{AppHandle, State};
use uuid::Uuid;

use app_core::app::memberships;
use app_core::data::app_users_communities::Membership;

use crate::command_error::CommandError;
use crate::device_accounts::DeviceAccounts;
use crate::sync;
use crate::user_auth::signed_in;

#[tauri::command]
pub async fn memberships_create(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    community_id: Uuid,
) -> Result<Membership, CommandError> {
    let (deployment, user) = signed_in(&device).await?;
    let membership = memberships::create(&deployment, &user, community_id).await?;
    sync::spawn(app);
    Ok(membership)
}

#[tauri::command]
pub async fn memberships_delete(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    community_id: Uuid,
) -> Result<(), CommandError> {
    let (deployment, user) = signed_in(&device).await?;
    memberships::delete(&deployment, &user, community_id).await?;
    sync::spawn(app);
    Ok(())
}
