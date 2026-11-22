//! The public website's pages the app links to, the terms and the
//! privacy policy, opened in the system browser, since the app never
//! shows a public page, see `docs/design/public-pages.md`: the active
//! account's server's, else the default server's.

use serde::Deserialize;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

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
    fn path(self) -> &'static str {
        match self {
            PublicPage::Terms => "/terms",
            PublicPage::Privacy => "/privacy",
        }
    }
}

/// Opens the public page in the system browser.
#[tauri::command]
pub async fn public_pages_open(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    page: PublicPage,
) -> Result<(), CommandError> {
    let server = device.active_or_default_server().await?;
    let url = server_urls::browser_page_url(&app, &server, page.path())?;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|error| format!("could not open the browser: {error}"))?;
    Ok(())
}
