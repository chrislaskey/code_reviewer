//! Stripe's pages, the checkout and the Customer Portal, which the
//! desktop opens in the system browser; a phone buys in its store.

use serde_json::Value;
use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

/// Opens the page the server answered in the system browser.
pub fn open(app: &AppHandle, url: Option<&Value>) -> Result<(), String> {
    let url = url
        .and_then(Value::as_str)
        .ok_or("The server answered no page to open")?;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|error| error.to_string())
}

/// A phone sells through its store's sheet, see `store_transactions`,
/// never Stripe's page; Android's store is not built yet.
pub fn refused_on_a_phone() -> Result<(), String> {
    if cfg!(target_os = "android") {
        return Err("On Android, purchases are not available yet".to_string());
    }
    if cfg!(mobile) {
        return Err("On a phone, plans and products are bought in its store".to_string());
    }
    Ok(())
}
