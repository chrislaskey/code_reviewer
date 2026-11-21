//! The public website's pages the app links to, the terms and the
//! privacy policy, opened in the system browser, since the app never
//! shows a public page, see `docs/design/public-pages.md`.

use serde::Deserialize;
use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

use crate::command_error::CommandError;
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
pub async fn public_pages_open(app: AppHandle, page: PublicPage) -> Result<(), CommandError> {
    let url = server_urls::browser_page_url(&app, page.path())?;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|error| format!("could not open the browser: {error}"))?;
    Ok(())
}
