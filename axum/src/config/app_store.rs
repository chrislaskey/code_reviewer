//! The server's App Store, chosen at start by its settings, all from the
//! environment and never committed: `APP_STORE_BUNDLE_ID`,
//! `APP_STORE_ISSUER_ID`, `APP_STORE_KEY_ID` and `APP_STORE_PRIVATE_KEY`
//! (the In-App Purchase key's `.p8`, its newlines as they are or as
//! `\n`), and `APP_STORE_APP_ID`, the app's Apple ID, without which only
//! the sandbox's purchases are taken. None set, the server sells nothing
//! in the iPhone app and its notifications route is a 404; some set and
//! not the others is a mistake, refused at start. See
//! `app_core::services::app_store` and `docs/design/purchasing-phones.md`.

use std::env;

use secrecy::SecretString;

use app_core::services::app_store::{AppStore, AppStoreSettings};

const REQUIRED: [&str; 4] = [
    "APP_STORE_BUNDLE_ID",
    "APP_STORE_ISSUER_ID",
    "APP_STORE_KEY_ID",
    "APP_STORE_PRIVATE_KEY",
];

pub fn open() -> Result<Option<AppStore>, String> {
    let values: Vec<Option<String>> = REQUIRED.iter().map(|name| setting(name)).collect();
    if values.iter().all(Option::is_none) {
        tracing::info!("app store: off, no APP_STORE_BUNDLE_ID");
        return Ok(None);
    }
    let missing: Vec<&str> = REQUIRED
        .iter()
        .zip(&values)
        .filter(|(_, value)| value.is_none())
        .map(|(name, _)| *name)
        .collect();
    if !missing.is_empty() {
        return Err(format!("app store: missing {}", missing.join(", ")));
    }
    let [bundle_id, issuer_id, key_id, private_key] =
        [0, 1, 2, 3].map(|at| values[at].clone().unwrap_or_default());
    let app_apple_id = match setting("APP_STORE_APP_ID") {
        Some(value) => Some(
            value
                .parse()
                .map_err(|_| format!("APP_STORE_APP_ID must be a number, got '{value}'"))?,
        ),
        None => None,
    };
    let settings = AppStoreSettings {
        bundle_id,
        app_apple_id,
        issuer_id,
        key_id,
        private_key: SecretString::from(private_key.replace("\\n", "\n")),
    };
    let app_store = AppStore::api(&settings).map_err(|error| error.to_string())?;
    match app_apple_id {
        Some(_) => tracing::info!("app store: on, production and sandbox"),
        None => tracing::info!("app store: on, sandbox only, no APP_STORE_APP_ID"),
    }
    Ok(Some(app_store))
}

fn setting(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.trim().is_empty())
}
