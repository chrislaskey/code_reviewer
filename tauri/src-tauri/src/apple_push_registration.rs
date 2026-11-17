//! A phone's Apple push token: asked of iOS (which asks the user's
//! permission the first time) and given to the paired computer, which
//! pushes each finished session to it through Apple. Sent again at each
//! launch, since the token changes when the app is reinstalled.
//! See decision 30 in `docs/decisions.md`.

use tauri::plugin::PermissionState;
use tauri::{AppHandle, Manager};
use tauri_plugin_notifications::NotificationsExt;

use crate::connection::BackendConnection;

/// Asks iOS for this phone's push token and gives it to the paired
/// computer. Asks the user's permission first if they haven't answered.
pub async fn register_for_apple_push(
    app: &AppHandle,
    connection: &BackendConnection,
) -> Result<(), String> {
    let client = connection.client()?;
    let token = app
        .notifications()
        .register_for_push_notifications()
        .await
        .map_err(|error| format!("Turning on notifications failed: {error}"))?;
    client
        .set_apple_push_token(&token)
        .await
        .map_err(|error| error.to_string())
}

/// Sends the token again, at launch and after pairing, if the user already
/// allowed notifications and the phone is paired, without asking anything.
pub async fn send_apple_push_token_if_allowed(app: AppHandle) {
    let allowed = matches!(
        app.notifications().permission_state().await,
        Ok(PermissionState::Granted)
    );
    let connection = app.state::<BackendConnection>();
    if !allowed || connection.client().is_err() {
        return;
    }
    if let Err(error) = register_for_apple_push(&app, &connection).await {
        log::warn!("Sending the push token failed: {error}");
    }
}
