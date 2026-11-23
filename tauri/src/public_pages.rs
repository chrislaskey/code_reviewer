//! The public website's pages the app links to, the terms and the
//! privacy policy, opened in the system browser, since the app never
//! shows a public page, see `docs/design/public-pages.md`: the active
//! account's server's, else the default server's, in the language the
//! app shows, when the pages are in it.

use serde::Deserialize;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use app_core::helpers::locales;

use crate::command_error::CommandError;
use crate::device_accounts::DeviceAccounts;
use crate::server_urls;

/// A public page the app may open.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicPage {
    Terms,
    Privacy,
}

impl PublicPage {
    /// The page's path in the language, `/es/terms`, or in English when
    /// the pages are not in it.
    fn path(self, locale: &str) -> String {
        let page = match self {
            PublicPage::Terms => "/terms",
            PublicPage::Privacy => "/privacy",
        };
        if locale != locales::DEFAULT && locales::is_supported(locale) {
            format!("/{locale}{page}")
        } else {
            page.to_string()
        }
    }
}

/// Opens the public page in the system browser.
#[tauri::command]
pub async fn public_pages_open(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    page: PublicPage,
    locale: String,
) -> Result<(), CommandError> {
    let server = device.active_or_default_server().await?;
    let url = server_urls::browser_page_url(&app, &server, &page.path(&locale))?;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|error| format!("could not open the browser: {error}"))?;
    Ok(())
}
