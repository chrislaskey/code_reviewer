//! A typed server's check, before the person signs in to a server other
//! than the build's default, see `device_accounts::servers` and
//! `docs/design/multi-server.md`. Plain http to this machine only in a
//! dev build.

use tauri::State;

use crate::command_error::CommandError;
use crate::device_accounts::DeviceAccounts;
use crate::device_accounts::servers::ServerChecked;

/// What the server at the URL says it is, and whether the default
/// server lists it; refused for another app or protocol version.
#[tauri::command]
pub async fn servers_get(
    device: State<'_, DeviceAccounts>,
    url: String,
) -> Result<ServerChecked, CommandError> {
    Ok(device.server_check(&url, cfg!(debug_assertions)).await?)
}
