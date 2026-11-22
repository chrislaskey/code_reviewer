//! The servers this device knows, `device_servers` in `device.db`, see
//! `docs/design/multi-server.md`: the build's default server, whose URL
//! `server_urls` finds at runtime and which is never stored, and any
//! other the person added. Each account is one server's, and the app
//! talks to the active account's server alone.

use serde::Serialize;
use uuid::Uuid;
use uuid::fmt::Hyphenated;

use app_core::services::web_server::Http;

use super::{DeviceAccounts, DeviceAccountsError};

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
