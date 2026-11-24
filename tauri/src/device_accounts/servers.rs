//! The servers this device knows, `device_servers` in `device.db`, see
//! `docs/design/multi-server.md`: the build's default server, whose URL
//! `server_urls` finds at runtime and which is never stored, and any
//! other the person added. Each account is one server's, and the app
//! talks to the active account's server alone. A typed URL is checked
//! before the person signs in there, `server_check`: what the server
//! says it is, and whether the default server lists it. Once the person
//! confirms it, it is kept, `server_keep`, and the sign-in starts there.
//! What a server offers, its settings, is kept on its row, shared by its
//! accounts and there before a sign-in; the default server's are the
//! build's defaults until it is first asked. Every sync answer carries
//! the server's settings version, and one that differs from the kept
//! one fetches them again, `server_settings_refresh`.

use serde::Serialize;
use tauri::{AppHandle, State};
use uuid::Uuid;
use uuid::fmt::Hyphenated;

use app_core::deployment::now;
use app_core::servers::settings::{ServerSettings, SettingsDocument};
use app_core::servers::{self, ListedServer, ServerName};
use app_core::services::mailer::mails::APP_NAME;
use app_core::services::web_server::Http;
use app_core::sync::SyncError;

use super::{DeviceAccounts, DeviceAccountsError};
use crate::command_error::CommandError;

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
    /// Its host and port, as the pages show it.
    pub host: String,
    pub name: String,
    /// What the server calls itself, for display only.
    pub display_name: String,
    pub app_identifier: String,
    pub protocol_version: u32,
    pub listing: Listing,
    /// What the server offers, with their version.
    pub settings: SettingsDocument,
}

/// A server this device knows.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DeviceServer {
    pub id: Uuid,
    /// The server's origin; `None` for the build's default server.
    pub url: Option<String>,
    /// What the server called itself when it was kept; `None` for the
    /// default server, which is never asked.
    pub display_name: Option<String>,
    /// What the server offers, as last fetched; the build's defaults
    /// until then.
    pub settings: ServerSettings,
    /// The version of the settings fetched; `None` until then.
    pub settings_version: Option<String>,
}

/// The server the pages are on, as they show it: the active account's,
/// else the default one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CurrentServer {
    pub display_name: String,
    /// Its host and port.
    pub host: String,
    /// The build's default server, the only one purchases go to.
    pub default: bool,
    /// What it offers, for the pages to show only that.
    pub settings: ServerSettings,
}

#[derive(sqlx::FromRow)]
struct ServerRow {
    id: Hyphenated,
    url: Option<String>,
    display_name: Option<String>,
    settings: Option<String>,
    settings_version: Option<String>,
}

impl From<ServerRow> for DeviceServer {
    fn from(row: ServerRow) -> DeviceServer {
        let settings = row
            .settings
            .as_deref()
            .map(serde_json::from_str::<ServerSettings>)
            .transpose()
            .unwrap_or_else(|error| {
                log::warn!("the settings kept for a server are not readable: {error}");
                None
            })
            .unwrap_or_default();
        DeviceServer {
            id: row.id.into_uuid(),
            url: row.url,
            display_name: row.display_name,
            settings,
            settings_version: row.settings_version,
        }
    }
}

impl DeviceAccounts {
    /// The build's default server.
    pub async fn default_server(&self) -> Result<DeviceServer, DeviceAccountsError> {
        let row: ServerRow = sqlx::query_as(
            "SELECT id, url, display_name, settings, settings_version FROM device_servers WHERE listed = 'default'"
        )
        .fetch_one(&self.list)
        .await?;
        Ok(row.into())
    }

    /// A server this device knows, by its id.
    pub async fn server_get(&self, id: Uuid) -> Result<DeviceServer, DeviceAccountsError> {
        let row: Option<ServerRow> = sqlx::query_as(
            "SELECT id, url, display_name, settings, settings_version FROM device_servers WHERE id = $1"
        )
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

    /// The server the pages are on: the active account's, else the
    /// default one, named for the app.
    pub async fn current_server(&self) -> Result<CurrentServer, DeviceAccountsError> {
        let server = self.active_or_default_server().await?;
        Ok(match &server.url {
            None => CurrentServer {
                display_name: APP_NAME.to_string(),
                host: super::host_of(&self.default_server_url),
                default: true,
                settings: server.settings,
            },
            Some(url) => CurrentServer {
                host: super::host_of(url),
                display_name: server.display_name.unwrap_or_else(|| super::host_of(url)),
                default: false,
                settings: server.settings,
            },
        })
    }

    /// The transport to a server.
    pub fn transport_for(&self, server: &DeviceServer) -> Result<Http, DeviceAccountsError> {
        let url = server.url.as_deref().unwrap_or(&self.default_server_url);
        Ok(Http::new(url)?)
    }

    /// Checks a typed URL: normalized to its origin, plain http only to
    /// this machine when `local_allowed`; the server there describes
    /// itself, which must be this app's at this app's protocol version,
    /// and answers its settings; and the default server is asked whether
    /// it lists that name at that origin. Nothing is kept.
    pub async fn server_check(
        &self,
        url: &str,
        local_allowed: bool,
    ) -> Result<ServerChecked, DeviceAccountsError> {
        let origin = servers::origin_of(url, local_allowed)?;
        let unreachable = |error: SyncError| DeviceAccountsError::ServerUnreachable {
            url: origin.clone(),
            reason: error.to_string(),
        };
        let server = Http::new(&origin)?;
        let description = server.servers_current_get().await.map_err(unreachable)?;
        description.compatible()?;
        let settings = server
            .servers_current_settings_get()
            .await
            .map_err(unreachable)?;
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
            host: super::host_of(&origin),
            url: origin,
            name: description.name,
            display_name: description.display_name,
            app_identifier: description.app_identifier,
            protocol_version: description.protocol_version,
            listing,
            settings,
        })
    }

    /// Keeps a server the person confirmed, with what its check found,
    /// in place of what an earlier check found; the default server's
    /// own row when it is the default server, with its settings.
    pub async fn server_keep(
        &self,
        checked: &ServerChecked,
    ) -> Result<DeviceServer, DeviceAccountsError> {
        if checked.listing == Listing::Default {
            let default = self.default_server().await?;
            return self
                .server_settings_keep(default.id, &checked.settings)
                .await;
        }
        let settings = serde_json::to_string(&checked.settings.settings)?;
        let listed = match checked.listing {
            Listing::Listed => "listed",
            Listing::Default | Listing::Unlisted | Listing::Unchecked => "unlisted",
        };
        let row: ServerRow = sqlx::query_as(
            "INSERT INTO device_servers
               (id, url, name, display_name, app_identifier, protocol_version, listed, checked_at,
                settings, settings_version)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
             ON CONFLICT (url) DO UPDATE SET name = excluded.name,
               display_name = excluded.display_name, app_identifier = excluded.app_identifier,
               protocol_version = excluded.protocol_version, listed = excluded.listed,
               checked_at = excluded.checked_at, settings = excluded.settings,
               settings_version = excluded.settings_version
             RETURNING id, url, display_name, settings, settings_version",
        )
        .bind(Uuid::now_v7().hyphenated())
        .bind(&checked.url)
        .bind(&checked.name)
        .bind(&checked.display_name)
        .bind(&checked.app_identifier)
        .bind(i64::from(checked.protocol_version))
        .bind(listed)
        .bind(now())
        .bind(settings)
        .bind(&checked.settings.settings_version)
        .fetch_one(&self.list)
        .await?;
        Ok(row.into())
    }

    /// Keeps a server's settings as it answered them, on its row and on
    /// the active account's server when it is that one.
    pub async fn server_settings_keep(
        &self,
        id: Uuid,
        document: &SettingsDocument,
    ) -> Result<DeviceServer, DeviceAccountsError> {
        let row: Option<ServerRow> = sqlx::query_as(
            "UPDATE device_servers SET settings = $1, settings_version = $2 WHERE id = $3
             RETURNING id, url, display_name, settings, settings_version",
        )
        .bind(serde_json::to_string(&document.settings)?)
        .bind(&document.settings_version)
        .bind(id.hyphenated())
        .fetch_optional(&self.list)
        .await?;
        let server = row
            .map(DeviceServer::from)
            .ok_or(DeviceAccountsError::UnknownServer { id })?;
        if let Some(active) = self.active.write().await.as_mut()
            && active.server.id == id
        {
            active.server = server.clone();
        }
        Ok(server)
    }

    /// Fetches the active account's server's settings again when the
    /// version its last answer carried is not the one kept; whether it
    /// did. Nothing is asked when no answer carried one.
    pub async fn server_settings_refresh(&self) -> Result<bool, DeviceAccountsError> {
        let Some((server, transport)) = self
            .active
            .read()
            .await
            .as_ref()
            .map(|active| (active.server.clone(), active.transport.clone()))
        else {
            return Ok(false);
        };
        let Some(seen) = transport.settings_version_seen() else {
            return Ok(false);
        };
        if server.settings_version.as_deref() == Some(seen.as_str()) {
            return Ok(false);
        }
        let document = transport.servers_current_settings_get().await?;
        self.server_settings_keep(server.id, &document).await?;
        Ok(true)
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

/// Keeps the server at the URL, checked again and allowed by the
/// build's `multiServer` setting, for a sign-in to start there.
#[tauri::command]
pub async fn device_servers_create(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    url: String,
) -> Result<DeviceServer, CommandError> {
    let checked = crate::servers::checked_and_allowed(&app, &device, &url).await?;
    Ok(device.server_keep(&checked).await?)
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
