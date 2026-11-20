//! The accounts commands: signing in and out on this device. A sign-in
//! happens in the system browser, on the web's own sign-in page, see
//! `docs/design/accounts-sign-in.md`: `accounts_sign_in_start` makes
//! the verifier, keeps it in `device.db`, and opens the browser at the
//! sign-in page `server_urls` names with the verifier's challenge; the
//! page, done,
//! shows a code the person pastes into the app, or on a desktop returns
//! it to `accounts_loopback`, and
//! `accounts_sign_in_finish` exchanges it with the verifier for the
//! session, which goes into the account's own file, see
//! `device_accounts`, which becomes the active one and syncs. The app
//! sees no password at a sign-in. A signed-in person makes or changes
//! theirs from the account page, which asks the server, so only when
//! online. A sign-out syncs first, so a change still
//! waiting reaches the server, then closes the account, whose data stays
//! for the next sign-in.

use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;

use crate::device_accounts::{DeviceAccounts, PendingSignIn, SignInFinished};
use crate::server_urls;
use crate::sync::{self, SyncServer};
use app_core::accounts::{self, PasswordUpdate, SecondFactor};

/// Keeps a verifier and opens the browser on the sign-in page, with the
/// email filled in for an account signed out on this device. On a
/// desktop the page returns the code to the app, see `accounts_loopback`.
#[tauri::command]
pub async fn accounts_sign_in_start(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    email: Option<String>,
) -> Result<(), String> {
    let (verifier, challenge) = accounts::verifier_and_challenge();
    device
        .sign_in_start(&verifier, email.as_deref())
        .await
        .map_err(|error| error.to_string())?;
    let url = sign_in_page_with(
        &server_urls::browser_sign_in_url(&app),
        &challenge,
        email.as_deref(),
        app_port(&app),
    );
    log::info!("opening the browser to sign in at {url}");
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|error| format!("could not open the browser: {error}"))
}

/// The port the page returns the code to; none on a phone, or when the
/// listener could not start, where the person pastes it.
#[cfg(desktop)]
fn app_port(app: &AppHandle) -> Option<u16> {
    crate::accounts_loopback::start(app)
        .inspect_err(|error| log::warn!("{error}; the code is pasted instead"))
        .ok()
}

#[cfg(mobile)]
fn app_port(_app: &AppHandle) -> Option<u16> {
    None
}

/// The sign-in waiting on the browser, for the accounts page to show
/// its dialog again after a relaunch.
#[tauri::command]
pub async fn accounts_sign_in_pending(
    device: State<'_, DeviceAccounts>,
) -> Result<Option<PendingSignIn>, String> {
    device
        .sign_in_pending()
        .await
        .map_err(|error| error.to_string())
}

/// Exchanges the code the browser showed, with the verifier kept at the
/// start, for the session, and keeps it as the active account; or, on a
/// device with a local user, offers to move what they made into the
/// account, see `local_users`.
#[tauri::command]
pub async fn accounts_sign_in_finish(
    app: AppHandle,
    code: String,
) -> Result<SignInFinished, String> {
    sign_in_finish(&app, code.trim()).await
}

/// The finish of a sign-in, with a pasted code or one the browser
/// returned to `accounts_loopback`.
pub async fn sign_in_finish(app: &AppHandle, code: &str) -> Result<SignInFinished, String> {
    let device = app.state::<DeviceAccounts>();
    let server = app.state::<SyncServer>();
    let verifier = device
        .sign_in_verifier()
        .await
        .map_err(|error| error.to_string())?
        .ok_or("No sign-in is waiting; start again")?;
    let session = accounts::sign_in_with_device_code_through(server.transport(), code, &verifier)
        .await
        .map_err(|error| error.to_string())?;
    let finished = {
        let _no_sync_meanwhile = server.hold().await;
        device
            .sign_in_finished(session)
            .await
            .map_err(|error| error.to_string())?
    };
    device
        .sign_in_clear()
        .await
        .map_err(|error| error.to_string())?;
    stop_loopback(app);
    crate::store_transactions::spawn_send_unfinished(app.clone());
    sync::spawn(app.clone());
    Ok(finished)
}

#[tauri::command]
pub async fn accounts_sign_in_cancel(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
) -> Result<(), String> {
    stop_loopback(&app);
    device.move_offered_forget().await;
    device
        .sign_in_clear()
        .await
        .map_err(|error| error.to_string())
}

fn stop_loopback(_app: &AppHandle) {
    #[cfg(desktop)]
    crate::accounts_loopback::stop(_app);
}

#[tauri::command]
pub async fn accounts_sign_out(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
) -> Result<(), String> {
    // Reported by the sync itself; what it could not push waits in the
    // account's file for the next sign-in.
    let _ = sync::run(&app).await;
    let _no_sync_meanwhile = server.hold().await;
    device
        .sign_out(server.transport())
        .await
        .map_err(|error| error.to_string())
}

/// What the signed-in user's account asks for after the emailed code,
/// asked of the server.
#[tauri::command]
pub async fn accounts_second_factors(
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
) -> Result<Vec<SecondFactor>, String> {
    let deployment = device.active().await.map_err(|error| error.to_string())?;
    accounts::second_factors_on_device(&deployment, server.transport())
        .await
        .map_err(|error| error.to_string())
}

/// The signed-in user's password, their first or a new one, kept by
/// the server.
#[tauri::command]
pub async fn accounts_password_update(
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
    current: Option<String>,
    password: String,
) -> Result<(), String> {
    let deployment = device.active().await.map_err(|error| error.to_string())?;
    let update = PasswordUpdate { current, password };
    accounts::password_update_on_device(&deployment, server.transport(), &update)
        .await
        .map_err(|error| error.to_string())
}

/// The sign-in page with the challenge, the email and the app's port in
/// its query.
fn sign_in_page_with(
    page: &str,
    challenge: &str,
    email: Option<&str>,
    app_port: Option<u16>,
) -> String {
    let mut url = format!("{page}?challenge={}", encode(challenge));
    if let Some(email) = email.filter(|email| !email.is_empty()) {
        url.push_str(&format!("&email={}", encode(email)));
    }
    if let Some(port) = app_port {
        url.push_str(&format!("&app_port={port}"));
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
        let url = sign_in_page_with(page, "abc123", Some(""), Some(53122));
        assert_eq!(
            url,
            "http://localhost:5002/accounts/sign-in?challenge=abc123&app_port=53122"
        );
    }
}
