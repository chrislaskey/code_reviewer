//! Where the build's default web server is, for the sync, and where its
//! sign-in page is, for the browser: from the environment, `SYNC_SERVER_URL` and
//! `BROWSER_SIGN_IN_URL`, which reaches the desktop app started from a
//! shell; else from `tauri.conf.json`, the `starter-pack` entry under
//! `plugins`, which is compiled into every build and which `bin/dev`
//! overrides with `--config` for the ports of the session, the only way
//! a value reaches a phone build, since no environment does; else the
//! dev server's port, and the sign-in page on the sync server itself,
//! where the pages are in production. Another server the person added
//! has its pages on itself, see `device_accounts::servers`.

use std::env;

use tauri::{AppHandle, Url};

use crate::device_accounts::servers::DeviceServer;

const CONFIG_ENTRY: &str = "starter-pack";
const DEFAULT_SYNC_SERVER_URL: &str = "http://localhost:4000";
const SIGN_IN_PATH: &str = "/accounts/sign-in";

/// The build's default web server.
pub fn sync_server_url(app: &AppHandle) -> String {
    pick(
        env::var("SYNC_SERVER_URL").ok(),
        configured(app, "syncServerUrl"),
        DEFAULT_SYNC_SERVER_URL.to_string(),
    )
}

/// The server's sign-in page, which the app opens in the browser.
pub fn browser_sign_in_url(app: &AppHandle, server: &DeviceServer) -> String {
    match &server.url {
        Some(url) => format!("{url}{SIGN_IN_PATH}"),
        None => pick(
            env::var("BROWSER_SIGN_IN_URL").ok(),
            configured(app, "browserSignInUrl"),
            format!("{}{SIGN_IN_PATH}", sync_server_url(app)),
        ),
    }
}

/// A page of the server's, such as `/terms`, which the app opens in the
/// browser: beside its sign-in page.
pub fn browser_page_url(
    app: &AppHandle,
    server: &DeviceServer,
    path: &str,
) -> Result<String, String> {
    page_beside(&browser_sign_in_url(app, server), path)
}

fn page_beside(sign_in_url: &str, path: &str) -> Result<String, String> {
    let mut url = Url::parse(sign_in_url)
        .map_err(|error| format!("the sign-in URL {sign_in_url} is not a URL: {error}"))?;
    url.set_path(path);
    url.set_query(None);
    url.set_fragment(None);
    Ok(url.to_string())
}

/// The environment's, else the config's, else the fallback; blanks
/// count as unset.
fn pick(from_environment: Option<String>, from_config: Option<String>, fallback: String) -> String {
    from_environment
        .into_iter()
        .chain(from_config)
        .map(|value| value.trim().to_string())
        .find(|value| !value.is_empty())
        .unwrap_or(fallback)
}

/// A value of the `starter-pack` entry in `tauri.conf.json`.
pub(crate) fn configured(app: &AppHandle, key: &str) -> Option<String> {
    app.config()
        .plugins
        .0
        .get(CONFIG_ENTRY)?
        .get(key)?
        .as_str()
        .map(String::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_urls_pick_the_environment_then_the_config_then_the_fallback() {
        let fallback = || "http://localhost:4000".to_string();
        assert_eq!(
            pick(
                Some(" http://a ".into()),
                Some("http://b".into()),
                fallback()
            ),
            "http://a"
        );
        assert_eq!(
            pick(Some("  ".into()), Some("http://b".into()), fallback()),
            "http://b",
            "a blank environment value is unset"
        );
        assert_eq!(pick(None, None, fallback()), "http://localhost:4000");
    }

    #[test]
    fn server_urls_browser_page_is_beside_the_sign_in_page() {
        assert_eq!(
            page_beside("http://localhost:5173/accounts/sign-in?x=1", "/terms").unwrap(),
            "http://localhost:5173/terms"
        );
        assert!(page_beside("not a url", "/terms").is_err());
    }
}
