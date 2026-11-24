//! ASWebAuthenticationSession for the iPhone app, called from the app's
//! Rust, see `ios/Sources/WebAuthSessionPlugin.swift` and
//! `docs/design/accounts-sign-in.md`: the system's sign-in sheet over
//! the app, which closes when a page in it goes to the app's scheme and
//! hands that URL back to this session alone. Written for this app in
//! place of `tauri-plugin-web-auth`, which does not keep its session,
//! and `tauri-plugin-auth-session`, which only JavaScript can call. On
//! any other platform every call answers `NotAvailable`.

use serde::Deserialize;
use tauri::plugin::{Builder, TauriPlugin};
use tauri::{Manager, Runtime};

#[cfg(target_os = "ios")]
tauri::ios_plugin_binding!(init_plugin_web_auth_session);

/// How the sheet closed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WebAuthSessionEnded {
    /// A page went to the scheme: the whole URL, with its query.
    Returned(String),
    /// The person closed the sheet.
    Cancelled,
}

#[derive(Debug, thiserror::Error)]
pub enum WebAuthSessionError {
    #[error("The sign-in sheet is only in the iPhone app")]
    NotAvailable,
    #[error("The sign-in sheet failed: {0}")]
    Failed(String),
}

pub type Result<T> = std::result::Result<T, WebAuthSessionError>;

/// The plugin's handle, managed by the app.
pub struct WebAuthSession<R: Runtime> {
    #[cfg(target_os = "ios")]
    handle: tauri::plugin::PluginHandle<R>,
    #[cfg(not(target_os = "ios"))]
    _runtime: std::marker::PhantomData<fn() -> R>,
}

/// `app.web_auth_session()`, for the app's commands.
pub trait WebAuthSessionExt<R: Runtime> {
    fn web_auth_session(&self) -> &WebAuthSession<R>;
}

impl<R: Runtime, T: Manager<R>> WebAuthSessionExt<R> for T {
    fn web_auth_session(&self) -> &WebAuthSession<R> {
        self.state::<WebAuthSession<R>>().inner()
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("web-auth-session")
        .setup(|app, api| {
            #[cfg(target_os = "ios")]
            let session = WebAuthSession {
                handle: api.register_ios_plugin(init_plugin_web_auth_session)?,
            };
            #[cfg(not(target_os = "ios"))]
            let session = {
                let _ = api;
                WebAuthSession::<R> {
                    _runtime: std::marker::PhantomData,
                }
            };
            app.manage(session);
            Ok(())
        })
        .build()
}

#[derive(Deserialize)]
struct OpenAnswer {
    outcome: String,
    url: Option<String>,
}

impl<R: Runtime> WebAuthSession<R> {
    /// Opens the sheet at the URL and waits until it closes, by a page
    /// going to `callback_scheme` or by the person.
    pub async fn open(&self, url: &str, callback_scheme: &str) -> Result<WebAuthSessionEnded> {
        let answer: OpenAnswer = self
            .call(
                "open",
                serde_json::json!({ "url": url, "callbackScheme": callback_scheme }),
            )
            .await?;
        match (answer.outcome.as_str(), answer.url) {
            ("returned", Some(url)) => Ok(WebAuthSessionEnded::Returned(url)),
            ("cancelled", _) => Ok(WebAuthSessionEnded::Cancelled),
            (other, _) => Err(WebAuthSessionError::Failed(format!(
                "an unknown outcome, {other}"
            ))),
        }
    }

    #[cfg(target_os = "ios")]
    async fn call<Answer: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        payload: serde_json::Value,
    ) -> Result<Answer> {
        self.handle
            .run_mobile_plugin_async(method, payload)
            .await
            .map_err(|error| WebAuthSessionError::Failed(error.to_string()))
    }

    #[cfg(not(target_os = "ios"))]
    async fn call<Answer: serde::de::DeserializeOwned>(
        &self,
        _method: &str,
        _payload: serde_json::Value,
    ) -> Result<Answer> {
        Err(WebAuthSessionError::NotAvailable)
    }
}
