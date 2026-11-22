//! The iPhone's sign-in: the web's sign-in page in the system's sign-in
//! sheet over the app, `tauri-plugin-web-auth-session`, rather than in
//! Safari, see `docs/design/accounts-sign-in.md`. The page, done, goes
//! to the app's scheme, its bundle identifier, with the device code;
//! the sheet closes and hands that URL to this session alone, and the
//! app finishes the sign-in as a pasted code does. The accounts page
//! hears how it ended through `accounts::sign_in_returned_tell`. A
//! sheet that closed with no sign-in ends it, as the page has no dialog
//! to paste a code in behind the sheet.

use tauri::{AppHandle, Manager};
use tauri_plugin_web_auth_session::{WebAuthSessionEnded, WebAuthSessionExt};

use crate::accounts::{self, SignInReturned};
use crate::device_accounts::DeviceAccounts;

/// Opens the sheet at the sign-in page, in place of any earlier one,
/// and finishes the sign-in of the verifier when it closes.
pub fn open(app: &AppHandle, url: String, verifier: String) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let scheme = app.config().identifier.clone();
        let returned = match app.web_auth_session().open(&url, &scheme).await {
            Ok(WebAuthSessionEnded::Returned(url)) => {
                accounts::sign_in_finish_returned(&app, &url).await
            }
            Ok(WebAuthSessionEnded::Cancelled) => SignInReturned::Cancelled,
            Err(error) => SignInReturned::Failed {
                error: error.to_string(),
            },
        };
        let finished = matches!(returned, SignInReturned::Finished { .. });
        if finished || sign_in_forget(&app, &verifier).await {
            accounts::sign_in_returned_tell(&app, returned);
        }
    });
}

/// Forgets the sign-in the sheet was for; whether it was still waiting,
/// not replaced by a newer sheet's, which cancels this one.
async fn sign_in_forget(app: &AppHandle, verifier: &str) -> bool {
    let device = app.state::<DeviceAccounts>();
    match device.sign_in_waiting().await {
        Ok(Some(waiting)) if waiting.verifier == verifier => {
            if let Err(error) = device.sign_in_clear().await {
                log::error!("could not forget the sign-in of the closed sheet: {error}");
            }
            true
        }
        Ok(Some(_newer)) => false,
        Ok(None) => true,
        Err(error) => {
            log::error!("could not read the sign-in of the closed sheet: {error}");
            true
        }
    }
}
