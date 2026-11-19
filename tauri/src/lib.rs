//! The desktop and mobile app: one window showing the Svelte pages
//! (`../svelte`), and a Tauri command per `app_core` function the pages
//! call, one module per resource, each command named resource first,
//! `posts_list`, since the commands are one namespace. The app keeps a
//! SQLite file per account signed in on it and one of them active, see
//! `device_accounts`; the other commands read and write the active one
//! as its user, see `current_user` and `accounts`, and it syncs with the web
//! server, see `sync`. A sign-in happens in the system browser, see
//! `accounts`. On macOS ⌃⌘T anywhere opens the quick picker, a second
//! window, see `quick_picker`.

mod accounts;
mod avatars;
#[cfg(any(target_os = "macos", target_os = "ios"))]
mod clipboard;
mod communities;
mod conflicts;
mod current_community;
mod current_user;
mod deletions;
mod device_accounts;
mod files;
mod local_users;
#[cfg(target_os = "macos")]
mod macos;
mod memberships;
mod pending_changes;
mod posts;
mod public_pages;
mod purchasing;
mod quick_picker;
mod server_urls;
#[cfg(mobile)]
mod store_sheet;
mod store_transactions;
mod sync;

use tauri::{Manager, WindowEvent};

pub(crate) const MAIN_WINDOW: &str = "main";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();
    #[cfg(target_os = "ios")]
    let builder = builder.plugin(tauri_plugin_storekit::init());
    builder
        .plugin(tauri_plugin_opener::init())
        .register_asynchronous_uri_scheme_protocol("files", |context, request, responder| {
            let app = context.app_handle().clone();
            tauri::async_runtime::spawn(async move {
                responder.respond(files::content_response(&app, request.uri()).await);
            });
        })
        .setup(|app| {
            device_accounts::open_and_manage(app)?;
            sync::setup(app)?;
            store_transactions::spawn_send_unfinished(app.handle().clone());
            quick_picker::register(app.handle())?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == MAIN_WINDOW && came_to_foreground(event) {
                sync::spawn_on_foreground(window.app_handle());
            }
        })
        .invoke_handler(tauri::generate_handler![
            current_user::current_user_get,
            current_user::current_user_update,
            current_community::current_community_get,
            current_community::current_community_update,
            avatars::avatars_create_upload,
            avatars::avatars_update,
            accounts::accounts_sign_in_start,
            accounts::accounts_sign_in_pending,
            accounts::accounts_sign_in_finish,
            accounts::accounts_sign_in_cancel,
            accounts::accounts_sign_out,
            accounts::accounts_second_factors,
            accounts::accounts_password_update,
            deletions::accounts_deletions_code_create,
            deletions::accounts_deletions_create,
            communities::communities_list,
            communities::communities_get,
            communities::communities_create,
            communities::communities_update,
            communities::communities_delete,
            memberships::memberships_join,
            memberships::memberships_leave,
            public_pages::public_pages_open,
            posts::posts_list,
            posts::posts_get,
            posts::posts_create,
            posts::posts_update,
            posts::posts_delete,
            files::files_list,
            files::files_list_with_sha256,
            files::files_get,
            files::files_create,
            files::files_update,
            files::files_delete,
            files::files_cache_get,
            files::files_cache_delete,
            files::files_copy_supported,
            files::files_copy,
            files::files_download_supported,
            files::files_download,
            sync::sync_run,
            sync::sync_status,
            sync::sync_set_offline,
            conflicts::conflicts_list,
            conflicts::conflicts_reapply,
            conflicts::conflicts_try_again,
            conflicts::conflicts_let_go,
            pending_changes::pending_changes_list,
            device_accounts::device_accounts_list,
            device_accounts::device_accounts_switch,
            device_accounts::device_accounts_deactivate,
            device_accounts::device_accounts_remove,
            local_users::local_users_can_create,
            local_users::local_users_create,
            local_users::local_users_move,
            local_users::local_users_keep_separate,
            quick_picker::quick_picker_list,
            quick_picker::quick_picker_paste,
            quick_picker::quick_picker_close,
            quick_picker::quick_picker_open_app,
            quick_picker::quick_picker_accessibility_request,
            purchasing::accounts_products_list,
            purchasing::accounts_products_get,
            purchasing::accounts_checkouts_create,
            purchasing::accounts_checkouts_get,
            purchasing::accounts_purchases_list,
            purchasing::accounts_purchases_get,
            purchasing::accounts_subscriptions_list,
            purchasing::accounts_subscriptions_get,
            purchasing::accounts_subscriptions_update,
            purchasing::accounts_subscriptions_portal_create,
            store_transactions::accounts_store_get,
            store_transactions::accounts_store_transactions_create,
            store_transactions::accounts_store_transactions_restore,
            store_transactions::accounts_subscriptions_manage_open
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
