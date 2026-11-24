//! The desktop and mobile app: one window showing the Svelte pages
//! (`../svelte`), and a Tauri command per `app_core` function the pages
//! call, one module per resource at the path of the core module it
//! calls, `app::posts`, each command named resource first, `posts_list`,
//! since the commands are one namespace, see
//! `docs/design/code-organization-axum-tauri-svelte.md`. The app keeps a
//! SQLite file per account signed in on it and one of them active, see
//! `device_accounts`; the other commands read and write the active one
//! as its user, see `user_auth` and `accounts`, and it syncs with the web
//! server, see `sync`. A sign-in happens in the system browser, see
//! `accounts`. On macOS ⌃⌘T anywhere opens the quick picker, a second
//! window, see `quick_picker`. On iOS the bar above the keyboard is
//! hidden, see `keyboard_accessory_bar`.

mod accounts;
mod app;
#[cfg(any(target_os = "macos", target_os = "ios"))]
mod clipboard;
mod command_error;
mod device_accounts;
#[cfg(target_os = "ios")]
mod keyboard_accessory_bar;
#[cfg(target_os = "macos")]
mod macos;
mod public_pages;
mod quick_picker;
mod server_urls;
mod servers;
mod sync;
mod user_auth;

use tauri::{Manager, WindowEvent};

pub(crate) const MAIN_WINDOW: &str = "main";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();
    #[cfg(target_os = "ios")]
    let builder = builder
        .plugin(tauri_plugin_storekit::init())
        .plugin(tauri_plugin_web_auth_session::init());
    builder
        .plugin(tauri_plugin_opener::init())
        .register_asynchronous_uri_scheme_protocol("files", |context, request, responder| {
            let app = context.app_handle().clone();
            tauri::async_runtime::spawn(async move {
                responder.respond(app::files::content_response(&app, request.uri()).await);
            });
        })
        .setup(|app| {
            #[cfg(target_os = "ios")]
            keyboard_accessory_bar::hide();
            device_accounts::open_and_manage(app)?;
            #[cfg(desktop)]
            app.manage(accounts::sign_in_loopback::AccountsLoopback::default());
            sync::setup(app)?;
            accounts::store_transactions::spawn_send_unfinished(app.handle().clone());
            quick_picker::register(app.handle())?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == MAIN_WINDOW && came_to_foreground(event) {
                sync::spawn_on_foreground(window.app_handle());
            }
        })
        .invoke_handler(tauri::generate_handler![
            app::current_user::current_user_get,
            app::current_user::current_user_update,
            app::current_community::current_community_get,
            app::current_community::current_community_update,
            app::avatars::uploads::avatars_uploads_create,
            app::avatars::avatars_update,
            accounts::accounts_sign_in_start,
            accounts::accounts_sign_in_pending,
            accounts::accounts_sign_in_finish,
            accounts::accounts_sign_in_cancel,
            accounts::accounts_sign_out,
            accounts::password::accounts_password_get,
            accounts::password::accounts_password_update,
            accounts::preferences::accounts_preferences_get,
            accounts::preferences::accounts_preferences_update,
            accounts::deletions::code::accounts_deletions_code_create,
            accounts::deletions::accounts_deletions_create,
            app::communities::communities_list,
            app::communities::communities_get,
            app::communities::communities_create,
            app::communities::communities_update,
            app::communities::communities_delete,
            app::memberships::memberships_create,
            app::memberships::memberships_delete,
            public_pages::public_pages_open,
            app::posts::posts_list,
            app::posts::posts_get,
            app::posts::posts_create,
            app::posts::posts_update,
            app::posts::posts_delete,
            app::files::files_list,
            app::files::duplicates::files_duplicates_list,
            app::files::files_get,
            app::files::files_create,
            app::files::files_update,
            app::files::files_delete,
            app::files::files_cache_get,
            app::files::files_cache_delete,
            app::files::files_copy_supported,
            app::files::files_copy,
            app::files::files_download_supported,
            app::files::files_download,
            sync::sync_run,
            sync::sync_status,
            sync::sync_set_offline,
            sync::conflicts::conflicts_list,
            sync::conflicts::conflicts_reapply,
            sync::conflicts::conflicts_try_again,
            sync::conflicts::conflicts_delete,
            sync::pending_changes::pending_changes_list,
            device_accounts::device_accounts_list,
            device_accounts::device_accounts_switch,
            device_accounts::device_accounts_deactivate,
            device_accounts::device_accounts_remove,
            device_accounts::servers::device_servers_create,
            servers::multi_server_get,
            servers::servers_current_get,
            servers::servers_get,
            app::local_users::local_users_can_create,
            app::local_users::local_users_create,
            app::local_users::local_users_move,
            app::local_users::local_users_keep_separate,
            quick_picker::quick_picker_files_list,
            quick_picker::quick_picker_paste,
            quick_picker::quick_picker_close,
            quick_picker::quick_picker_open_app,
            quick_picker::quick_picker_accessibility_request,
            accounts::products::accounts_products_list,
            accounts::products::accounts_products_get,
            accounts::checkouts::accounts_checkouts_create,
            accounts::checkouts::accounts_checkouts_get,
            accounts::purchases::accounts_purchases_list,
            accounts::purchases::accounts_purchases_get,
            accounts::subscriptions::accounts_subscriptions_list,
            accounts::subscriptions::accounts_subscriptions_get,
            accounts::subscriptions::accounts_subscriptions_update,
            accounts::subscriptions::portal::accounts_subscriptions_portal_create,
            accounts::store_transactions::accounts_store_get,
            accounts::store_transactions::accounts_store_transactions_create,
            accounts::store_transactions::accounts_store_transactions_restore,
            accounts::subscriptions::accounts_subscriptions_manage_open
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// The window gained focus, or the app resumed on a phone, which Tauri
/// only reports on mobile builds.
fn came_to_foreground(event: &WindowEvent) -> bool {
    #[cfg(mobile)]
    if matches!(event, WindowEvent::Resumed) {
        return true;
    }
    matches!(event, WindowEvent::Focused(true))
}
