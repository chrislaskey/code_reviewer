//! The iPhone's sign-in: the web's sign-in page in the system's sign-in
//! sheet over the app, `tauri-plugin-web-auth-session`, rather than in
//! Safari, see `docs/design/accounts-sign-in.md`. The page, done, goes
//! to the app's scheme, its bundle identifier, with the device code;
//! the sheet closes and hands that URL to this session alone, and the
//! app finishes the sign-in as a pasted code does. The accounts page
//! hears how it ended through `accounts::sign_in_returned_tell`. A
//! sheet the person closed leaves the sign-in waiting, so a code they
//! copied in it can still be pasted.

use tauri::AppHandle;
use tauri_plugin_web_auth_session::{WebAuthSessionEnded, WebAuthSessionExt};

use crate::accounts::{self, SignInReturned};

/// Opens the sheet at the sign-in page, in place of any earlier one,
/// and finishes the sign-in when it closes.
pub fn open(app: &AppHandle, url: String) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let scheme = app.config().identifier.clone();
        let returned = match app.web_auth_session().open(&url, &scheme).await {
            Ok(WebAuthSessionEnded::Returned(url)) => {
                accounts::sign_in_finish_returned(&app, &url).await
            }
            Ok(WebAuthSessionEnded::Cancelled) => return,
            Err(error) => SignInReturned::Failed {
                error: error.to_string(),
            },
        };
        accounts::sign_in_returned_tell(&app, returned);
    });
}
