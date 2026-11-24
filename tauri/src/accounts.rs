//! The accounts commands: signing in and out on this device. A sign-in
//! happens in the system browser, on the web's own sign-in page, see
//! `docs/design/accounts-sign-in.md`: `accounts_sign_in_start` makes
//! the verifier, keeps it in `device.db` with the server it is on, and
//! opens the browser at that server's sign-in page, see `server_urls`,
//! with the verifier's challenge; the
//! page, done, shows a code the person pastes into the app, or returns
//! it by itself: on a desktop to `sign_in_loopback`, on an iPhone
//! through the sign-in sheet of `sign_in_sheet`; and
//! `accounts_sign_in_finish` exchanges it with the verifier, on the same
//! server, for the session, which goes into the account's own file, see
//! `device_accounts`, which becomes the active one and syncs. The app
//! sees no password at a sign-in. A signed-in person makes or changes
//! theirs from the account page, which asks the server, so only when
//! online. A sign-out syncs first, so a change still
//! waiting reaches the server, then closes the account, whose data stays
//! for the next sign-in.
//!
//! The accounts app's other commands are its submodules, each named as
//! the web server's route module for the same resource. A phone buys in
//! its store, `store_transactions` through `store_sheet`; the desktop on
//! Stripe's pages, `stripe_pages`.

pub mod checkouts;
pub mod deletions;
pub mod password;
pub mod plans;
pub mod preferences;
pub mod products;
pub mod purchases;
#[cfg(desktop)]
pub mod sign_in_loopback;
#[cfg(target_os = "ios")]
pub mod sign_in_sheet;
#[cfg(mobile)]
pub mod store_sheet;
pub mod store_transactions;
pub mod stripe_pages;
pub mod subscriptions;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State, Url};
#[cfg(not(target_os = "ios"))]
use tauri_plugin_opener::OpenerExt;
use uuid::Uuid;

use crate::command_error::CommandError;
use crate::device_accounts::servers::DeviceServer;
use crate::device_accounts::{DeviceAccounts, PendingSignIn, SignInFinished};
use crate::server_urls;
use crate::sync::{self, SyncServer};
use app_core::accounts;

const SIGN_IN_RETURNED_EVENT: &str = "accounts-sign-in-returned";

/// How the page hands the code back to the app by itself.
#[derive(Clone, Debug, PartialEq, Eq)]
enum AppReturn {
    /// To `sign_in_loopback`'s listener on this computer.
    #[cfg_attr(not(desktop), allow(dead_code))]
    Port(u16),
    /// To the app's scheme, which closes the iPhone's sign-in sheet.
    #[cfg_attr(not(target_os = "ios"), allow(dead_code))]
    Scheme(String),
}

/// What the accounts page is told when the code came back by itself.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum SignInReturned {
    Finished {
        finished: SignInFinished,
    },
    Failed {
        error: String,
    },
    /// The person closed the iPhone's sign-in sheet, which ends the sign-in.
    #[cfg_attr(not(target_os = "ios"), allow(dead_code))]
    Cancelled,
}

/// Where the sign-in page opened.
#[derive(Clone, Debug, Serialize)]
pub struct SignInStarted {
    /// In the iPhone's sign-in sheet over the app, which returns the code
    /// itself, so the page shows no dialog to paste it in.
    pub in_sheet: bool,
}

/// Keeps a verifier and opens the sign-in page of the server, the
/// default one when none is named, with the email filled in for an
/// account signed out on this device: in the browser, or on an iPhone
/// in the sign-in sheet over the app.
#[tauri::command]
pub async fn accounts_sign_in_start(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    email: Option<String>,
    server_id: Option<Uuid>,
) -> Result<SignInStarted, CommandError> {
    let server = match server_id {
        Some(id) => device.server_get(id).await?,
        None => device.default_server().await?,
    };
    let (verifier, challenge) = accounts::verifier_and_challenge();
    device
        .sign_in_start(server.id, &verifier, email.as_deref())
        .await?;
    let url = sign_in_page_with(
        &server_urls::browser_sign_in_url(&app, &server),
        &challenge,
        email.as_deref(),
        app_return(&app, &server),
    );
    sign_in_page_open(&app, url, verifier)?;
    Ok(SignInStarted {
        in_sheet: cfg!(target_os = "ios"),
    })
}

/// Where the page returns the code to; none on Android, or when the
/// listener could not start, where the person pastes it.
#[cfg(desktop)]
fn app_return(app: &AppHandle, server: &DeviceServer) -> Option<AppReturn> {
    crate::accounts::sign_in_loopback::start(app, server)
        .inspect_err(|error| log::warn!("{error}; the code is pasted instead"))
        .ok()
        .map(AppReturn::Port)
}

#[cfg(target_os = "ios")]
fn app_return(app: &AppHandle, _server: &DeviceServer) -> Option<AppReturn> {
    Some(AppReturn::Scheme(app.config().identifier.clone()))
}

#[cfg(target_os = "android")]
fn app_return(_app: &AppHandle, _server: &DeviceServer) -> Option<AppReturn> {
    None
}

#[cfg(target_os = "ios")]
fn sign_in_page_open(app: &AppHandle, url: String, verifier: String) -> Result<(), String> {
    log::info!("opening the sign-in sheet at {url}");
    crate::accounts::sign_in_sheet::open(app, url, verifier);
    Ok(())
}

#[cfg(not(target_os = "ios"))]
fn sign_in_page_open(app: &AppHandle, url: String, _verifier: String) -> Result<(), String> {
    log::info!("opening the browser to sign in at {url}");
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|error| format!("could not open the browser: {error}"))
}

/// The sign-in waiting on the browser, for the accounts page to show
/// its dialog again after a relaunch.
#[tauri::command]
pub async fn accounts_sign_in_pending(
    device: State<'_, DeviceAccounts>,
) -> Result<Option<PendingSignIn>, CommandError> {
    Ok(device.sign_in_pending().await?)
}

/// Exchanges the code the browser showed, with the verifier kept at the
/// start, for the session, and keeps it as the active account; or, on a
/// device with a local user, offers to move what they made into the
/// account, see `app::local_users`.
#[tauri::command]
pub async fn accounts_sign_in_finish(
    app: AppHandle,
    code: String,
) -> Result<SignInFinished, CommandError> {
    Ok(sign_in_finish(&app, code.trim()).await?)
}

/// The finish of a sign-in, with a pasted code or one that came back
/// by itself, see `sign_in_finish_returned`.
pub async fn sign_in_finish(app: &AppHandle, code: &str) -> Result<SignInFinished, String> {
    let device = app.state::<DeviceAccounts>();
    let sync_server = app.state::<SyncServer>();
    let waiting = device
        .sign_in_waiting()
        .await
        .map_err(|error| error.to_string())?
        .ok_or("No sign-in is waiting; start again")?;
    let transport = device
        .server_get(waiting.server_id)
        .await
        .and_then(|server| device.transport_for(&server))
        .map_err(|error| error.to_string())?;
    let session = accounts::sign_in_with_device_code_through(&transport, code, &waiting.verifier)
        .await
        .map_err(|error| error.to_string())?;
    let finished = {
        let _no_sync_meanwhile = sync_server.hold().await;
        device
            .sign_in_finished(waiting.server_id, session)
            .await
            .map_err(|error| error.to_string())?
    };
    device
        .sign_in_clear()
        .await
        .map_err(|error| error.to_string())?;
    stop_loopback(app);
    crate::accounts::store_transactions::spawn_send_unfinished(app.clone());
    sync::spawn(app.clone());
    Ok(finished)
}

#[tauri::command]
pub async fn accounts_sign_in_cancel(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
) -> Result<(), CommandError> {
    stop_loopback(&app);
    device.pending_move_forget().await;
    Ok(device.sign_in_clear().await?)
}

/// The finish of a sign-in with the URL the page handed back, which
/// carries the code.
pub async fn sign_in_finish_returned(app: &AppHandle, url: &str) -> SignInReturned {
    let Some(code) = device_code_in(url) else {
        return SignInReturned::Failed {
            error: "The sign-in page sent back no code; paste it instead".to_string(),
        };
    };
    match sign_in_finish(app, &code).await {
        Ok(finished) => SignInReturned::Finished { finished },
        Err(error) => SignInReturned::Failed { error },
    }
}

/// Tells the accounts page, with `accounts-sign-in-returned`.
pub fn sign_in_returned_tell(app: &AppHandle, returned: SignInReturned) {
    if let Err(error) = app.emit(SIGN_IN_RETURNED_EVENT, returned) {
        log::error!("could not tell the page how the sign-in ended: {error}");
    }
}

/// The `code` in a URL's query.
pub fn device_code_in(url: &str) -> Option<String> {
    let url = Url::parse(url).ok()?;
    url.query_pairs()
        .find(|(name, _)| name == "code")
        .map(|(_, code)| code.trim().to_string())
        .filter(|code| !code.is_empty())
}

fn stop_loopback(_app: &AppHandle) {
    #[cfg(desktop)]
    crate::accounts::sign_in_loopback::stop(_app);
}

#[tauri::command]
pub async fn accounts_sign_out(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
) -> Result<(), CommandError> {
    // Reported by the sync itself; what it could not push waits in the
    // account's file for the next sign-in.
    let _ = sync::run(&app).await;
    let _no_sync_meanwhile = server.hold().await;
    let (_, transport) = device.active_with_transport().await?;
    Ok(device.sign_out(&transport).await?)
}

/// The sign-in page with the challenge, the email and where to return
/// the code in its query.
fn sign_in_page_with(
    page: &str,
    challenge: &str,
    email: Option<&str>,
    app_return: Option<AppReturn>,
) -> String {
    let mut url = format!("{page}?challenge={}", encode(challenge));
    if let Some(email) = email.filter(|email| !email.is_empty()) {
        url.push_str(&format!("&email={}", encode(email)));
    }
    match app_return {
        Some(AppReturn::Port(port)) => url.push_str(&format!("&app_port={port}")),
        Some(AppReturn::Scheme(scheme)) => {
            url.push_str(&format!("&app_scheme={}", encode(&scheme)))
        }
        None => {}
    }
    url
}

/// Percent-encodes a query value: everything but the unreserved
/// characters.
fn encode(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accounts_sign_in_page_with_carries_the_challenge_and_the_email_encoded() {
        let page = "http://localhost:5002/accounts/sign-in";
        let url = sign_in_page_with(page, "abc123", Some("alice+1@example.com"), None);
        assert_eq!(
            url,
            "http://localhost:5002/accounts/sign-in?challenge=abc123&email=alice%2B1%40example.com"
        );
        let url = sign_in_page_with(page, "abc123", Some(""), Some(AppReturn::Port(53122)));
        assert_eq!(
            url,
            "http://localhost:5002/accounts/sign-in?challenge=abc123&app_port=53122"
        );
        let url = sign_in_page_with(
            page,
            "abc123",
            None,
            Some(AppReturn::Scheme("com.starterpack.app".to_string())),
        );
        assert_eq!(
            url,
            "http://localhost:5002/accounts/sign-in?challenge=abc123&app_scheme=com.starterpack.app"
        );
    }

    #[test]
    fn accounts_device_code_in_reads_the_code_from_the_query() {
        assert_eq!(
            device_code_in("http://127.0.0.1:53122/?code=abc123"),
            Some("abc123".to_string())
        );
        assert_eq!(
            device_code_in("com.starterpack.app://accounts/sign-in?code=abc123"),
            Some("abc123".to_string())
        );
        assert_eq!(device_code_in("http://127.0.0.1:53122/favicon.ico"), None);
        assert_eq!(device_code_in("http://127.0.0.1:53122/?code="), None);
    }
}
