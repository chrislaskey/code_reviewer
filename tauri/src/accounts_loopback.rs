//! The desktop's return from a browser sign-in: a listener on
//! `127.0.0.1` the web page redirects to with the device code, so the
//! person need not paste it, see `docs/design/accounts-sign-in.md`.
//! `tauri-plugin-oauth` runs the listener, which answers the first
//! request with a redirect to the web's "back to the app" page and
//! hands its URL here; the code in it is useless without the verifier
//! this app kept, so a stray request costs nothing. The app then
//! finishes the sign-in as a pasted code does, comes to the front, and
//! tells the accounts page, see `accounts::sign_in_returned_tell`.

use std::sync::Mutex;

use tauri::{AppHandle, Manager};
use tauri_plugin_oauth::OauthConfig;

use crate::{MAIN_WINDOW, accounts, server_urls};

const BACK_TO_APP_PATH: &str = "/accounts/back-to-app";

/// The port of the listener waiting for the browser, one at a time.
#[derive(Default)]
pub struct AccountsLoopback {
    port: Mutex<Option<u16>>,
}

/// Starts a listener for the browser to return the code to, in place of
/// any earlier one, and answers its port.
pub fn start(app: &AppHandle) -> Result<u16, String> {
    stop(app);
    let config = OauthConfig {
        ports: None,
        response: None,
        redirect_uri: Some(server_urls::browser_page_url(app, BACK_TO_APP_PATH)?.into()),
    };
    let returned_to = app.clone();
    let port = tauri_plugin_oauth::start_with_config(config, move |url| {
        returned(returned_to.clone(), url)
    })
    .map_err(|error| format!("could not listen for the browser: {error}"))?;
    if let Ok(mut current) = app.state::<AccountsLoopback>().port.lock() {
        *current = Some(port);
    }
    Ok(port)
}

/// Stops the listener waiting for the browser, if one is.
pub fn stop(app: &AppHandle) {
    let port = match app.state::<AccountsLoopback>().port.lock() {
        Ok(mut current) => current.take(),
        Err(_) => None,
    };
    if let Some(port) = port {
        // Fails only when the listener already ended, which is fine.
        let _ = tauri_plugin_oauth::cancel(port);
    }
}

/// The listener's one request: finishes the sign-in with its code.
fn returned(app: AppHandle, url: String) {
    if let Ok(mut current) = app.state::<AccountsLoopback>().port.lock() {
        *current = None;
    }
    tauri::async_runtime::spawn(async move {
        let returned = accounts::sign_in_finish_returned(&app, &url).await;
        bring_to_front(&app);
        accounts::sign_in_returned_tell(&app, returned);
    });
}

fn bring_to_front(app: &AppHandle) {
    let Some(window) = app.get_webview_window(MAIN_WINDOW) else {
        return;
    };
    let shown = window
        .unminimize()
        .and_then(|()| window.show())
        .and_then(|()| window.set_focus());
    if let Err(error) = shown {
        log::warn!("could not bring the app to the front: {error}");
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::sync::mpsc;
    use std::time::Duration;

    use super::*;

    #[test]
    fn accounts_loopback_listener_redirects_the_browser_and_hands_over_the_url() {
        let (sender, received) = mpsc::channel();
        let config = OauthConfig {
            ports: None,
            response: None,
            redirect_uri: Some("https://server.test/accounts/back-to-app".into()),
        };
        let port = tauri_plugin_oauth::start_with_config(config, move |url| {
            let _ = sender.send(url);
        })
        .unwrap();

        let mut browser = TcpStream::connect(("127.0.0.1", port)).unwrap();
        // In one write, as a browser sends it: the listener reads once.
        let request = format!("GET /?code=abc123 HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n");
        browser.write_all(request.as_bytes()).unwrap();
        let mut answer = String::new();
        browser.read_to_string(&mut answer).unwrap();

        assert!(answer.starts_with("HTTP/1.1 302"), "{answer}");
        assert!(answer.contains("Location: https://server.test/accounts/back-to-app"));
        let url = received.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(accounts::device_code_in(&url), Some("abc123".to_string()));
    }
}
