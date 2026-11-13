//! The devices paired for remote access, each with a token of its own, kept
//! in `paired_devices.json` (readable only by the user). The file holds a
//! SHA-256 of each token, not the token: the pairing code shown once at
//! pairing is the only copy.

use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use crate::tokens::create_token;

const MAX_DEVICE_NAME_LENGTH: usize = 60;

/// A device paired for remote access.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairedDevice {
    pub id: String,
    pub name: String,
    /// RFC 3339.
    pub paired_at: String,
}

#[derive(Clone, Serialize, Deserialize)]
struct StoredDevice {
    #[serde(flatten)]
    device: PairedDevice,
    token_sha256: String,
}

/// The paired devices, read from and written to their file.
pub struct PairedDevices {
    path: PathBuf,
    devices: Mutex<Vec<StoredDevice>>,
}

impl PairedDevices {
    /// The devices in the file at `path`, or none when there is no file.
    pub fn open(path: PathBuf) -> Result<Self, String> {
        let devices = match fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text)
                .map_err(|error| format!("{}: {error}", path.display()))?,
            Err(error) if error.kind() == ErrorKind::NotFound => Vec::new(),
            Err(error) => return Err(format!("{}: {error}", path.display())),
        };
        Ok(Self {
            path,
            devices: Mutex::new(devices),
        })
    }

    /// Every paired device, oldest first.
    pub fn list(&self) -> Vec<PairedDevice> {
        self.lock()
            .iter()
            .map(|stored| stored.device.clone())
            .collect()
    }

    /// Pairs a device named `name`, returning it and its new token.
    pub fn create(&self, name: &str) -> Result<(PairedDevice, SecretString), String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("give the device a name".to_string());
        }
        if name.chars().count() > MAX_DEVICE_NAME_LENGTH {
            return Err(format!(
                "a device name is at most {MAX_DEVICE_NAME_LENGTH} characters"
            ));
        }
        let token = create_token().map_err(|error| error.to_string())?;
        let id = create_token().map_err(|error| error.to_string())?;
        let device = PairedDevice {
            id: id.expose_secret()[..16].to_string(),
            name: name.to_string(),
            paired_at: chrono::Utc::now().to_rfc3339(),
        };
        let mut devices = self.lock();
        devices.push(StoredDevice {
            device: device.clone(),
            token_sha256: sha256_hex(token.expose_secret()),
        });
        if let Err(error) = write_private_file(&self.path, &devices) {
            devices.pop();
            return Err(error);
        }
        Ok((device, token))
    }

    /// Unpairs the device with `id`; its token stops working at once.
    pub fn revoke(&self, id: &str) -> Result<(), String> {
        let mut devices = self.lock();
        let Some(index) = devices.iter().position(|stored| stored.device.id == id) else {
            return Err("no paired device has that id".to_string());
        };
        let removed = devices.remove(index);
        if let Err(error) = write_private_file(&self.path, &devices) {
            devices.insert(index, removed);
            return Err(error);
        }
        Ok(())
    }

    /// The device whose token `token` is, compared in constant time.
    pub fn find_device_for_token(&self, token: &str) -> Option<PairedDevice> {
        let sent_sha256 = sha256_hex(token);
        self.lock()
            .iter()
            .find(|stored| bool::from(stored.token_sha256.as_bytes().ct_eq(sent_sha256.as_bytes())))
            .map(|stored| stored.device.clone())
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<StoredDevice>> {
        // A panic while holding the lock leaves the list whole, so a poisoned one is still usable.
        self.devices
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

fn sha256_hex(text: &str) -> String {
    Sha256::digest(text.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Writes `devices` beside `path` and moves it into place, so a crash never
/// leaves half a file.
fn write_private_file(path: &Path, devices: &[StoredDevice]) -> Result<(), String> {
    let text = serde_json::to_string_pretty(devices).map_err(|error| error.to_string())?;
    let partial_path = path.with_extension("json.partial");
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options
        .open(&partial_path)
        .map_err(|error| format!("{}: {error}", partial_path.display()))?;
    file.write_all(text.as_bytes())
        .map_err(|error| format!("{}: {error}", partial_path.display()))?;
    fs::rename(&partial_path, path).map_err(|error| format!("{}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_in(directory: &tempfile::TempDir) -> PairedDevices {
        PairedDevices::open(directory.path().join("paired_devices.json")).unwrap()
    }

    #[test]
    fn a_paired_devices_token_finds_it() {
        let directory = tempfile::tempdir().unwrap();
        let devices = open_in(&directory);

        let (device, token) = devices.create("  My phone ").unwrap();

        assert_eq!(device.name, "My phone");
        assert_eq!(
            devices.find_device_for_token(token.expose_secret()),
            Some(device)
        );
        assert_eq!(devices.find_device_for_token("not-a-token"), None);
    }

    #[test]
    fn paired_devices_outlive_a_restart_but_their_tokens_are_not_stored() {
        let directory = tempfile::tempdir().unwrap();
        let (device, token) = open_in(&directory).create("My phone").unwrap();

        let reopened = open_in(&directory);

        assert_eq!(reopened.list(), vec![device]);
        assert!(
            reopened
                .find_device_for_token(token.expose_secret())
                .is_some()
        );
        let text = fs::read_to_string(directory.path().join("paired_devices.json")).unwrap();
        assert!(!text.contains(token.expose_secret()));
    }

    #[test]
    fn a_revoked_devices_token_stops_working() {
        let directory = tempfile::tempdir().unwrap();
        let devices = open_in(&directory);
        let (phone, phone_token) = devices.create("Phone").unwrap();
        let (tablet, tablet_token) = devices.create("Tablet").unwrap();

        devices.revoke(&phone.id).unwrap();

        assert_eq!(
            devices.find_device_for_token(phone_token.expose_secret()),
            None
        );
        assert_eq!(
            devices.find_device_for_token(tablet_token.expose_secret()),
            Some(tablet.clone())
        );
        assert_eq!(open_in(&directory).list(), vec![tablet]);
    }

    #[test]
    fn revoking_an_unknown_device_is_an_error() {
        let directory = tempfile::tempdir().unwrap();

        assert!(open_in(&directory).revoke("nope").is_err());
    }

    #[test]
    fn a_device_needs_a_name_of_reasonable_length() {
        let directory = tempfile::tempdir().unwrap();
        let devices = open_in(&directory);

        assert!(devices.create("   ").is_err());
        assert!(devices.create(&"x".repeat(61)).is_err());
        assert!(devices.list().is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn the_file_is_readable_only_by_the_user() {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().unwrap();
        open_in(&directory).create("Phone").unwrap();

        let path = directory.path().join("paired_devices.json");
        let mode = fs::metadata(path).unwrap().permissions().mode();

        assert_eq!(mode & 0o777, 0o600);
    }
}
