//! The servers this device knows, `device_servers` in `device.db`, see
//! `docs/design/multi-server.md`: the build's default server, whose URL
//! `server_urls` finds at runtime and which is never stored, and any
//! other the person added. Each account is one server's, and the app
//! talks to the active account's server alone. A typed URL is checked
//! before the person signs in there, `server_check`: what the server
//! says it is, and whether the default server lists it.

use serde::Serialize;
use uuid::Uuid;
use uuid::fmt::Hyphenated;

use app_core::servers::{self, ListedServer, ServerName};
use app_core::services::web_server::Http;
use app_core::sync::SyncError;

use super::{DeviceAccounts, DeviceAccountsError};

/// Whether the default server lists a server, as a check found it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Listing {
    /// The build's default server itself.
    Default,
    Listed,
    Unlisted,
    /// The default server could not be asked.
    Unchecked,
}

/// A server a typed URL reaches, as a check found it, for the person
/// to confirm before signing in there.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ServerChecked {
    /// The origin the URL normalizes to.
    pub url: String,
    pub name: String,
    /// What the server calls itself, for display only.
    pub display_name: String,
    pub listing: Listing,
}

/// A server this device knows.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DeviceServer {
    pub id: Uuid,
    /// The server's origin; `None` for the build's default server.
    pub url: Option<String>,
}

#[derive(sqlx::FromRow)]
struct ServerRow {
    id: Hyphenated,
    url: Option<String>,
}

impl From<ServerRow> for DeviceServer {
    fn from(row: ServerRow) -> DeviceServer {
        DeviceServer {
            id: row.id.into_uuid(),
            url: row.url,
        }
    }
}

impl DeviceAccounts {
    /// The build's default server.
    pub async fn default_server(&self) -> Result<DeviceServer, DeviceAccountsError> {
        let row: ServerRow =
            sqlx::query_as("SELECT id, url FROM device_servers WHERE listed = 'default'")
                .fetch_one(&self.list)
                .await?;
        Ok(row.into())
    }

    /// A server this device knows, by its id.
    pub async fn server_get(&self, id: Uuid) -> Result<DeviceServer, DeviceAccountsError> {
        let row: Option<ServerRow> =
            sqlx::query_as("SELECT id, url FROM device_servers WHERE id = $1")
                .bind(id.hyphenated())
                .fetch_optional(&self.list)
                .await?;
        row.map(DeviceServer::from)
            .ok_or(DeviceAccountsError::UnknownServer { id })
    }

    /// The server an account is on: its own, or the default server's
    /// for a local user, who is on none.
    pub(super) async fn server_of(
        &self,
        server_id: Option<Uuid>,
    ) -> Result<DeviceServer, DeviceAccountsError> {
        match server_id {
            Some(id) => self.server_get(id).await,
            None => self.default_server().await,
        }
    }

    /// The transport to a server.
    pub fn transport_for(&self, server: &DeviceServer) -> Result<Http, DeviceAccountsError> {
        let url = server.url.as_deref().unwrap_or(&self.default_server_url);
        Ok(Http::new(url)?)
    }

    /// Checks a typed URL: normalized to its origin, plain http only to
    /// this machine when `local_allowed`; the server there describes
    /// itself, which must be this app's at this app's protocol version;
    /// and the default server is asked whether it lists that name at
    /// that origin. Nothing is kept.
    pub async fn server_check(
        &self,
        url: &str,
        local_allowed: bool,
    ) -> Result<ServerChecked, DeviceAccountsError> {
        let origin = servers::origin_of(url, local_allowed)?;
        let description = Http::new(&origin)?
            .servers_current_get()
            .await
            .map_err(|error| DeviceAccountsError::ServerUnreachable {
                url: origin.clone(),
                reason: error.to_string(),
            })?;
        description.compatible()?;
        let name = ServerName::parse(&description.name)?;
        let default_origin = servers::origin_of(&self.default_server_url, true).ok();
        let listing = if default_origin.as_deref() == Some(origin.as_str()) {
            Listing::Default
        } else {
            let answer = self
                .transport_for(&self.default_server().await?)?
                .servers_get(name.as_str(), &origin)
                .await;
            listing_of(&origin, answer)
        };
        Ok(ServerChecked {
            url: origin,
            name: description.name,
            display_name: description.display_name,
            listing,
        })
    }

    /// Makes the default server's row when the device has none yet.
    pub(super) async fn default_server_create(&self) -> Result<(), DeviceAccountsError> {
        sqlx::query(
            "INSERT INTO device_servers (id, url, listed)
             SELECT $1, NULL, 'default'
             WHERE NOT EXISTS (SELECT 1 FROM device_servers WHERE listed = 'default')",
        )
        .bind(Uuid::now_v7().hyphenated())
        .execute(&self.list)
        .await?;
        Ok(())
    }
}

/// The listing the default server answered: listed only when its entry
/// is at the origin the device connects to.
fn listing_of(origin: &str, answer: Result<Option<ListedServer>, SyncError>) -> Listing {
    match answer {
        Ok(Some(listed)) if listed.url == origin => Listing::Listed,
        Ok(_) => Listing::Unlisted,
        Err(error) => {
            log::warn!("could not ask the default server whether it lists {origin}: {error}");
            Listing::Unchecked
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use app_core::app::identity::APP_IDENTIFIER;

    #[test]
    fn device_servers_listing_is_listed_only_at_the_origin_connected_to() {
        let origin = "https://acme.example.com";
        let entry = |url: &str| ListedServer {
            name: "acme".to_string(),
            url: url.to_string(),
        };
        assert_eq!(listing_of(origin, Ok(Some(entry(origin)))), Listing::Listed);
        assert_eq!(
            listing_of(origin, Ok(Some(entry("https://elsewhere.example.com")))),
            Listing::Unlisted
        );
        assert_eq!(listing_of(origin, Ok(None)), Listing::Unlisted);
        assert_eq!(
            listing_of(origin, Err(SyncError::Transport("offline".to_string()))),
            Listing::Unchecked
        );
    }

    #[test]
    fn device_servers_app_identifier_is_the_bundle_identifier() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../../tauri.conf.json")).expect("the config");
        assert_eq!(config["identifier"], APP_IDENTIFIER);
    }
}
