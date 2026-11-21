//! The current user's commands: who is signed in on this device, as
//! their own pages show them, and their new name, see
//! `app_core::app::users`.

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, State};
use uuid::Uuid;

use app_core::accounts::online;
use app_core::app::users::{self, Kind, UserError};
use app_core::data::app_users::User;

use crate::command_error::CommandError;
use crate::device_accounts::{DeviceAccounts, DeviceAccountsError};
use crate::sync;
use crate::user_auth::signed_in;

/// The user signed in as their own pages show them: with the email of
/// their account, which the device keeps in its list of accounts and
/// their synced row does not carry, whether they are a local user, on
/// this device only, and their plan as the device last heard it, until
/// its period ends, see `app_core::accounts::online`.
#[derive(Serialize)]
pub struct CurrentUserShown {
    #[serde(flatten)]
    pub user: User,
    pub email: String,
    pub kind: Kind,
    pub plan: Value,
    pub payment_failed_subscription_id: Option<Uuid>,
}

/// The user signed in, or none when nobody is; an error only when the
/// device could not tell.
#[tauri::command]
pub async fn current_user_get(
    device: State<'_, DeviceAccounts>,
) -> Result<Option<CurrentUserShown>, CommandError> {
    let deployment = match device.active().await {
        Ok(deployment) => deployment,
        Err(DeviceAccountsError::NobodyActive) => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let user = match users::signed_in(&deployment).await {
        Ok(user) => user,
        Err(UserError::NotSignedIn) => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    shown(&device, user).await.map(Some)
}

/// The signed-in user's new name, written on this device and synced
/// when it next can.
#[tauri::command]
pub async fn current_user_update(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    name: String,
) -> Result<CurrentUserShown, CommandError> {
    let (deployment, user) = signed_in(&device).await?;
    let user = users::update(&deployment, &user, &name).await?;
    sync::spawn(app);
    shown(&device, user).await
}

async fn shown(device: &DeviceAccounts, user: User) -> Result<CurrentUserShown, CommandError> {
    let accounts = device.list().await?;
    let account = accounts
        .into_iter()
        .find(|account| account.user_id == user.id)
        .ok_or("The signed-in account is not in this device's list")?;
    let kind = match account.local {
        true => Kind::Local,
        false => Kind::Full,
    };
    let deployment = device.active().await?;
    let kept = online::plan_kept(&deployment).await?;
    Ok(CurrentUserShown {
        user,
        email: account.email,
        kind,
        plan: kept.as_ref().map_or(Value::Null, |kept| kept.plan.clone()),
        payment_failed_subscription_id: kept.and_then(|kept| kept.payment_failed_subscription_id),
    })
}
