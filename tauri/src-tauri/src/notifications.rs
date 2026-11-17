//! Shows the desktop's session notifications: asks the server for them
//! again and again, and shows each as a system notification. Clicking one
//! brings the window to the front and tells the pages to open the session.

use std::sync::Arc;
use std::time::Duration;

use code_reviewer_client::BackendClient;
use code_reviewer_server::SessionNotification;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::{ActionPerformed, NotificationExt};

/// How long to wait before asking again after a failed request.
const RETRY_DELAY: Duration = Duration::from_secs(5);
/// The key in a notification's extra data holding its session's name.
const SESSION_NAME_KEY: &str = "session_name";
/// The event the pages listen for, with the session's name, to open its page.
const SESSION_NOTIFICATION_CLICKED_EVENT: &str = "session_notification_clicked";
const MAIN_WINDOW_LABEL: &str = "main";

/// Runs for as long as the app does.
pub async fn show_session_notifications(app: AppHandle, client: Arc<BackendClient>) {
    let clicked_app = app.clone();
    if let Err(error) = app
        .notification()
        .on_action(move |performed| open_clicked_session(&clicked_app, performed))
    {
        log::warn!("Listening for clicks on notifications failed: {error}");
    }
    loop {
        match client.wait_for_session_notifications().await {
            Ok(notifications) => {
                for notification in notifications {
                    show(&app, &notification);
                }
            }
            Err(error) => {
                log::warn!("Asking for session notifications failed: {error}");
                tokio::time::sleep(RETRY_DELAY).await;
            }
        }
    }
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
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
    if let Err(error) = app.emit_to(
        MAIN_WINDOW_LABEL,
        SESSION_NOTIFICATION_CLICKED_EVENT,
        session_name,
    ) {
        log::warn!("Opening the clicked session failed: {error}");
    }
}
