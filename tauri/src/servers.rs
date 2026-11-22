//! Signing in to a server other than the build's default, see
//! `device_accounts::servers` and `docs/design/multi-server.md`, as far
//! as the build's `multiServer` setting, in the `starter-pack` entry of
//! `tauri.conf.json`, allows: `none`, the default, never; `listed_only`,
//! only servers the default server lists; `any`, any server, an unlisted
//! one shown as such. Plain http to this machine only in a dev build.

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::command_error::CommandError;
use crate::device_accounts::DeviceAccounts;
use crate::device_accounts::servers::{Listing, ServerChecked};
use crate::server_urls;

const OWN_SERVER_ONLY: &str = "This app signs in to its own server alone";

/// Which servers other than the default one the app may sign in to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MultiServer {
    None,
    ListedOnly,
    Any,
}

impl MultiServer {
    fn parse(value: Option<&str>) -> MultiServer {
        match value.map(str::trim) {
            None | Some("") | Some("none") => MultiServer::None,
            Some("listed_only") => MultiServer::ListedOnly,
            Some("any") => MultiServer::Any,
            Some(other) => {
                log::warn!("multiServer '{other}' is not none, listed_only or any: none it is");
                MultiServer::None
            }
        }
    }

    /// Whether a server the check found so may be signed in to; why not
    /// when it may not.
    pub fn allows(self, listing: Listing) -> Result<(), String> {
        match (self, listing) {
            (_, Listing::Default) => Ok(()),
            (MultiServer::None, _) => Err(OWN_SERVER_ONLY.to_string()),
            (MultiServer::ListedOnly, Listing::Listed) | (MultiServer::Any, _) => Ok(()),
            (MultiServer::ListedOnly, Listing::Unlisted) => {
                Err("This server is not on the app's list of servers".to_string())
            }
            (MultiServer::ListedOnly, Listing::Unchecked) => {
                Err("This server cannot be checked now; try again when online".to_string())
            }
        }
    }
}

/// The build's `multiServer` setting.
pub fn multi_server(app: &AppHandle) -> MultiServer {
    MultiServer::parse(server_urls::configured(app, "multiServer").as_deref())
}

/// The server at the URL, checked, when the setting allows signing in
/// there.
pub async fn checked_and_allowed(
    app: &AppHandle,
    device: &DeviceAccounts,
    url: &str,
) -> Result<ServerChecked, CommandError> {
    let setting = multi_server(app);
    if setting == MultiServer::None {
        return Err(OWN_SERVER_ONLY.into());
    }
    let checked = device.server_check(url, cfg!(debug_assertions)).await?;
    setting.allows(checked.listing)?;
    Ok(checked)
}

/// The build's `multiServer` setting, for the accounts page to offer
/// another server or not.
#[tauri::command]
pub fn multi_server_get(app: AppHandle) -> MultiServer {
    multi_server(&app)
}

/// What the server at the URL says it is, and whether the default
/// server lists it; refused for another app or protocol version, or as
/// the setting says.
#[tauri::command]
pub async fn servers_get(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    url: String,
) -> Result<ServerChecked, CommandError> {
    checked_and_allowed(&app, &device, &url).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn servers_multi_server_is_none_unless_set_to_a_known_value() {
        assert_eq!(MultiServer::parse(None), MultiServer::None);
        assert_eq!(
            MultiServer::parse(Some("listed_only")),
            MultiServer::ListedOnly
        );
        assert_eq!(MultiServer::parse(Some(" any ")), MultiServer::Any);
        assert_eq!(MultiServer::parse(Some("all")), MultiServer::None);
    }

    #[test]
    fn servers_multi_server_allows_as_set() {
        for setting in [MultiServer::None, MultiServer::ListedOnly, MultiServer::Any] {
            assert!(setting.allows(Listing::Default).is_ok(), "{setting:?}");
        }
        assert!(MultiServer::None.allows(Listing::Listed).is_err());
        assert!(MultiServer::ListedOnly.allows(Listing::Listed).is_ok());
        assert!(MultiServer::ListedOnly.allows(Listing::Unlisted).is_err());
        assert!(
            MultiServer::ListedOnly.allows(Listing::Unchecked).is_err(),
            "refused when it cannot check"
        );
        assert!(MultiServer::Any.allows(Listing::Unlisted).is_ok());
        assert!(MultiServer::Any.allows(Listing::Unchecked).is_ok());
    }
}
