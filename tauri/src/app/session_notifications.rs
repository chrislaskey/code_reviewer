//! Shows the desktop's session notifications: asks the server for them
//! again and again, as the signed-in person, and shows each as a system
//! notification. Clicking one brings the window to the front and tells
//! the pages to open the session. While nobody is signed in, it waits.

use std::time::Duration;

use serde::Deserialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::{ActionPerformed, NotificationExt};

use app_core::app::sessions::online;

use crate::MAIN_WINDOW;
use crate::device_accounts::DeviceAccounts;

/// How long to wait before asking again after a failed request, or while
/// nobody is signed in.
const RETRY_DELAY: Duration = Duration::from_secs(5);
/// The key in a notification's extra data holding its session's name.
const SESSION_NAME_KEY: &str = "session_name";
/// The event the pages listen for, with the session's name, to open its page.
const SESSION_NOTIFICATION_CLICKED_EVENT: &str = "session_notification_clicked";

/// One agent finishing, as the server answers it.
#[derive(Deserialize)]
struct SessionNotification {
    session_name: String,
    title: String,
    body: String,
}

/// Starts showing the notifications, for as long as the app runs.
pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(show_session_notifications(app));
}

async fn show_session_notifications(app: AppHandle) {
    let clicked_app = app.clone();
    if let Err(error) = app
        .notification()
        .on_action(move |performed| open_clicked_session(&clicked_app, performed))
    {
        log::warn!("Listening for clicks on notifications failed: {error}");
    }
    loop {
        match ask_for_notifications(&app).await {
            Ok(notifications) => {
                for notification in notifications {
                    show(&app, &notification);
                }
            }
            Err(error) => {
                log::debug!("Asking for session notifications failed: {error}");
                tokio::time::sleep(RETRY_DELAY).await;
            }
        }
    }
}

async fn ask_for_notifications(app: &AppHandle) -> Result<Vec<SessionNotification>, String> {
    let device = app.state::<DeviceAccounts>();
    let (deployment, transport) = device
        .active_with_transport()
        .await
        .map_err(|error| error.to_string())?;
    let answer = online::notifications_waiting_list(&deployment, &transport)
        .await
        .map_err(|error| error.to_string())?;
    serde_json::from_value(answer).map_err(|error| error.to_string())
}

fn show(app: &AppHandle, notification: &SessionNotification) {
    log::info!("Showing the notification for {}", notification.session_name);
    let shown = app
        .notification()
        .builder()
        .title(&notification.title)
        .body(&notification.body)
        .extra(SESSION_NAME_KEY, &notification.session_name)
        .show();
    if let Err(error) = shown {
        log::warn!("Showing a notification failed: {error}");
    }
}

fn open_clicked_session(app: &AppHandle, performed: &ActionPerformed) {
    if performed.action_id() != "tap" {
        return;
    }
    let Some(session_name) = performed
        .notification()
        .and_then(|notification| notification.extra().get(SESSION_NAME_KEY))
        .and_then(|value| value.as_str())
    else {
        return;
    };
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
    if let Err(error) = app.emit_to(
        MAIN_WINDOW,
        SESSION_NOTIFICATION_CLICKED_EVENT,
        session_name,
    ) {
        log::warn!("Opening the clicked session failed: {error}");
    }
}
