//! Remote access (`src/remote_access/`): on the desktop, turning the
//! tailnet listener on and off and pairing devices; on a phone, pairing with
//! a computer. Each desktop command is one call to the backend server.

use code_reviewer_server::{CreatedPairedDevice, RemoteAccessStatus};
use tauri::State;

use crate::connection::{AppPlatform, BackendConnection, PairingStatus};

/// Whether this app runs the backend itself (desktop) or reaches one (mobile).
#[tauri::command]
pub fn get_app_platform() -> AppPlatform {
    if cfg!(mobile) {
        AppPlatform::Mobile
    } else {
        AppPlatform::Desktop
    }
}

/// Whether remote access is on, its URL, and the paired devices.
#[tauri::command]
pub async fn get_remote_access_status(
    connection: State<'_, BackendConnection>,
) -> Result<RemoteAccessStatus, String> {
    connection
        .client()?
        .get_remote_access_status()
        .await
        .map_err(|error| error.to_string())
}

/// Turns remote access on or off.
#[tauri::command]
pub async fn set_remote_access_enabled(
    connection: State<'_, BackendConnection>,
    enabled: bool,
) -> Result<RemoteAccessStatus, String> {
    connection
        .client()?
        .set_remote_access_enabled(enabled)
        .await
        .map_err(|error| error.to_string())
}

/// Pairs a device named `name`, returning its pairing code, shown once.
#[tauri::command]
pub async fn create_paired_device(
    connection: State<'_, BackendConnection>,
    name: String,
) -> Result<CreatedPairedDevice, String> {
    connection
        .client()?
        .create_paired_device(&name)
        .await
        .map_err(|error| error.to_string())
}

/// Unpairs a device; its token stops working at once.
#[tauri::command]
pub async fn revoke_paired_device(
    connection: State<'_, BackendConnection>,
    id: String,
) -> Result<RemoteAccessStatus, String> {
    connection
        .client()?
        .revoke_paired_device(&id)
        .await
        .map_err(|error| error.to_string())
}

/// The computer this phone is paired with, if any.
#[tauri::command]
pub fn get_pairing_status(connection: State<'_, BackendConnection>) -> PairingStatus {
    connection.get_pairing_status()
}

/// Pairs this phone with the computer in a pairing code.
#[tauri::command]
pub async fn pair_with_computer(
    connection: State<'_, BackendConnection>,
    code: String,
) -> Result<PairingStatus, String> {
    connection.pair(&code).await
}

/// Forgets the computer this phone is paired with.
#[tauri::command]
pub fn unpair_from_computer(
    connection: State<'_, BackendConnection>,
) -> Result<PairingStatus, String> {
    connection.unpair()
}
