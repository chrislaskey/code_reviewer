//! A phone's Apple push token: asked of iOS (which asks the person's
//! permission the first time) and given to the server, as the signed-in
//! person, which pushes each finished session to it through Apple. Sent
//! again at each launch, since the token changes when the app is
//! reinstalled. See decision 30 in `docs/decisions.md`.

use tauri::plugin::PermissionState;
use tauri::{AppHandle, Manager};
use tauri_plugin_notifications::NotificationsExt;

use app_core::app::sessions::online;

use crate::device_accounts::DeviceAccounts;

/// Asks iOS for this phone's push token and gives it to the server. Asks
/// the person's permission first if they haven't answered.
pub async fn register_for_apple_push(app: &AppHandle) -> Result<(), String> {
    let device = app.state::<DeviceAccounts>();
    let (deployment, transport) = device
        .active_with_transport()
        .await
        .map_err(|error| error.to_string())?;
    let token = app
        .notifications()
        .register_for_push_notifications()
        .await
        .map_err(|error| format!("Turning on notifications failed: {error}"))?;
    online::apple_push_token_update(&deployment, &transport, &token)
        .await
        .map_err(|error| error.to_string())?;
    Ok(())
}

/// Sends the token again at launch, if the person already allowed
/// notifications and is signed in, without asking anything.
pub fn spawn_send_if_allowed(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let allowed = matches!(
            app.notifications().permission_state().await,
            Ok(PermissionState::Granted)
        );
        let signed_in = app
            .state::<DeviceAccounts>()
            .active_with_transport()
            .await
            .is_ok();
        if !allowed || !signed_in {
            return;
        }
        if let Err(error) = register_for_apple_push(&app).await {
            log::warn!("Sending the push token failed: {error}");
        }
    });
}
