//! Which backend server the app's commands go to. On the desktop it is the
//! server the app started on localhost; on a phone, the computer it was
//! paired with, or none yet.

use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use code_reviewer_client::BackendClient;
use code_reviewer_server::PairingCode;
use serde::Serialize;

/// Whether this app runs the backend itself or reaches one remotely.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AppPlatform {
    Desktop,
    Mobile,
}

/// The phone's pairing, as its pairing page shows it.
#[derive(Clone, Debug, Serialize)]
pub struct PairingStatus {
    /// The computer's URL, when paired.
    pub url: Option<String>,
}

/// The client the commands use, if there is one yet.
pub struct BackendConnection {
    client: RwLock<Option<Arc<BackendClient>>>,
    /// Where a phone keeps its pairing code; `None` on the desktop.
    pairing_code_path: Option<PathBuf>,
}

impl BackendConnection {
    /// The desktop's connection, to the server it started.
    #[cfg(any(desktop, test))]
    pub fn to_local_server(client: BackendClient) -> Self {
        Self {
            client: RwLock::new(Some(Arc::new(client))),
            pairing_code_path: None,
        }
    }

    /// A phone's connection, to the computer of the pairing code saved at
    /// `pairing_code_path`, or unpaired when there is none.
    #[cfg(any(mobile, test))]
    pub fn from_saved_pairing_code(pairing_code_path: PathBuf) -> Self {
        let client = std::fs::read_to_string(&pairing_code_path)
            .ok()
            .and_then(|text| PairingCode::decode(&text).ok())
            .map(|code| Arc::new(BackendClient::from_pairing_code(&code)));
        Self {
            client: RwLock::new(client),
            pairing_code_path: Some(pairing_code_path),
        }
    }

    /// The client, or an error when this phone isn't paired yet.
    pub fn client(&self) -> Result<Arc<BackendClient>, String> {
        self.read()
            .clone()
            .ok_or_else(|| "This device isn't paired with a computer yet.".to_string())
    }

    /// The URL of the computer this phone is paired with.
    pub fn get_pairing_status(&self) -> PairingStatus {
        PairingStatus {
            url: self
                .read()
                .as_ref()
                .map(|client| client.base_url().to_string()),
        }
    }

    /// Pairs this phone with the computer in `code`: checks the computer
    /// answers with the code's token, then saves the code.
    pub async fn pair(&self, code: &str) -> Result<PairingStatus, String> {
        let path = self.pairing_code_path_or_error()?;
        let code = PairingCode::decode(code)?;
        let client = BackendClient::from_pairing_code(&code);
        client
            .get_session_settings()
            .await
            .map_err(|error| format!("The computer didn't accept the code: {error}"))?;
        write_private_file(path, &code.encode())?;
        *self.write() = Some(Arc::new(client));
        Ok(self.get_pairing_status())
    }

    /// Forgets the computer this phone was paired with.
    pub fn unpair(&self) -> Result<PairingStatus, String> {
        let path = self.pairing_code_path_or_error()?;
        match std::fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("{}: {error}", path.display())),
        }
        *self.write() = None;
        Ok(self.get_pairing_status())
    }

    fn pairing_code_path_or_error(&self) -> Result<&Path, String> {
        self.pairing_code_path
            .as_deref()
            .ok_or_else(|| "Only the mobile app pairs with a computer.".to_string())
    }

    fn read(&self) -> std::sync::RwLockReadGuard<'_, Option<Arc<BackendClient>>> {
        // A panic while holding the lock leaves an Option whole, so a poisoned one is still usable.
        self.client
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn write(&self) -> std::sync::RwLockWriteGuard<'_, Option<Arc<BackendClient>>> {
        self.client
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

fn write_private_file(path: &Path, text: &str) -> Result<(), String> {
    use std::io::Write;
    if let Some(directory) = path.parent() {
        std::fs::create_dir_all(directory)
            .map_err(|error| format!("{}: {error}", directory.display()))?;
    }
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options
        .open(path)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    file.write_all(text.as_bytes())
        .map_err(|error| format!("{}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use secrecy::SecretString;

    use super::*;

    #[test]
    fn a_saved_pairing_code_connects_a_phone_at_launch() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("pairing_code");
        let code = PairingCode {
            url: "https://my-mac.tail1234.ts.net:7443".to_string(),
            token: SecretString::from("abc"),
        };
        write_private_file(&path, &code.encode()).unwrap();

        let connection = BackendConnection::from_saved_pairing_code(path);

        assert_eq!(
            connection.get_pairing_status().url.as_deref(),
            Some("https://my-mac.tail1234.ts.net:7443")
        );
        assert!(connection.client().is_ok());
    }

    #[test]
    fn an_unpaired_phone_has_no_client() {
        let directory = tempfile::tempdir().unwrap();

        let connection =
            BackendConnection::from_saved_pairing_code(directory.path().join("pairing_code"));

        assert_eq!(connection.get_pairing_status().url, None);
        assert!(connection.client().is_err());
    }

    #[test]
    fn unpairing_forgets_the_saved_code() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("pairing_code");
        let code = PairingCode {
            url: "https://my-mac.tail1234.ts.net:7443".to_string(),
            token: SecretString::from("abc"),
        };
        write_private_file(&path, &code.encode()).unwrap();
        let connection = BackendConnection::from_saved_pairing_code(path.clone());

        connection.unpair().unwrap();

        assert!(connection.client().is_err());
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn pairing_rejects_text_that_isnt_a_code() {
        let directory = tempfile::tempdir().unwrap();
        let connection =
            BackendConnection::from_saved_pairing_code(directory.path().join("pairing_code"));

        let error = connection.pair("hello").await.unwrap_err();

        assert!(error.contains("pairing code"), "{error}");
        assert!(connection.client().is_err());
    }

    #[tokio::test]
    async fn the_desktop_doesnt_pair() {
        let connection = BackendConnection::to_local_server(BackendClient::new(
            "http://127.0.0.1:1".to_string(),
            SecretString::from("abc"),
        ));

        assert!(connection.pair("cr1-x").await.is_err());
        assert!(connection.client().is_ok());
    }
}
