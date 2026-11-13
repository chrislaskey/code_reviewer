//! Remote access: the tailnet listener, on only while the user has turned
//! it on, and the devices paired to use it. Managed from the desktop app
//! through routes the localhost listener alone serves.

use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::Arc;

use axum::middleware;
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, oneshot};

use crate::access::{AccessTokens, ListenerAccess, require_listener_access};
use crate::backend::Backend;
use crate::http_api::tailnet_router;
use crate::paired_devices::{PairedDevice, PairedDevices};
use crate::pairing_code::PairingCode;
use crate::tailnet_listener::TailnetListener;
use crate::tailscale::TailscaleCommand;

/// The port the tailnet listener uses unless the settings file names another.
pub const DEFAULT_TAILNET_PORT: u16 = 7443;

/// What the user chose, kept in `remote_access.json`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteAccessSettings {
    pub enabled: bool,
    pub port: u16,
}

impl Default for RemoteAccessSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            port: DEFAULT_TAILNET_PORT,
        }
    }
}

/// Remote access as the desktop app shows it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RemoteAccessStatus {
    /// Whether the user turned remote access on.
    pub enabled: bool,
    /// The address paired devices use, while the tailnet listener runs.
    pub url: Option<String>,
    /// Why remote access is on but the listener isn't running.
    pub problem: Option<String>,
    pub paired_devices: Vec<PairedDevice>,
}

/// A device just paired, with the one copy of its pairing code.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreatedPairedDevice {
    pub device: PairedDevice,
    pub pairing_code: String,
}

struct RunningTailnetListener {
    url: String,
    stop_sender: oneshot::Sender<()>,
}

struct RemoteAccessState {
    settings: RemoteAccessSettings,
    running: Option<RunningTailnetListener>,
    problem: Option<String>,
}

/// Turns the tailnet listener on and off, and pairs devices.
pub struct RemoteAccess {
    settings_path: PathBuf,
    backend: Arc<Backend>,
    paired_devices: Arc<PairedDevices>,
    state: Mutex<RemoteAccessState>,
}

impl RemoteAccess {
    /// Remote access with the settings and paired devices kept in `data_directory`.
    pub fn open(data_directory: &std::path::Path, backend: Arc<Backend>) -> Result<Self, String> {
        let settings_path = data_directory.join("remote_access.json");
        let settings = read_settings(&settings_path)?;
        let paired_devices =
            PairedDevices::open(data_directory.join("paired_devices.json")).map(Arc::new)?;
        Ok(Self {
            settings_path,
            backend,
            paired_devices,
            state: Mutex::new(RemoteAccessState {
                settings,
                running: None,
                problem: None,
            }),
        })
    }

    /// The status, starting the listener first when remote access is on but
    /// it isn't running (Tailscale may have connected since the last try).
    pub async fn get_status(&self) -> RemoteAccessStatus {
        let mut state = self.state.lock().await;
        if state.settings.enabled && state.running.is_none() {
            self.start_listener(&mut state).await;
        }
        self.status_of(&state)
    }

    /// Turns remote access on or off, remembering the choice.
    pub async fn set_enabled(&self, enabled: bool) -> Result<RemoteAccessStatus, String> {
        let mut state = self.state.lock().await;
        let settings = RemoteAccessSettings {
            enabled,
            ..state.settings.clone()
        };
        write_settings(&self.settings_path, &settings)?;
        state.settings = settings;
        if enabled {
            if state.running.is_none() {
                self.start_listener(&mut state).await;
            }
        } else {
            stop_listener(&mut state);
            state.problem = None;
        }
        Ok(self.status_of(&state))
    }

    /// Pairs a device named `name`. Needs the listener running, since the
    /// pairing code holds its URL.
    pub async fn create_paired_device(&self, name: &str) -> Result<CreatedPairedDevice, String> {
        let state = self.state.lock().await;
        let url = state
            .running
            .as_ref()
            .map(|running| running.url.clone())
            .ok_or(
                "turn on remote access, and wait for it to be reachable, before pairing a device",
            )?;
        let (device, token) = self.paired_devices.create(name)?;
        let pairing_code = PairingCode { url, token }.encode();
        Ok(CreatedPairedDevice {
            device,
            pairing_code,
        })
    }

    /// Unpairs a device; its token stops working at once.
    pub async fn revoke_paired_device(&self, id: &str) -> Result<RemoteAccessStatus, String> {
        self.paired_devices.revoke(id)?;
        let state = self.state.lock().await;
        Ok(self.status_of(&state))
    }

    async fn start_listener(&self, state: &mut RemoteAccessState) {
        match self.start_tailnet_listener(state.settings.port).await {
            Ok(running) => {
                log::info!("serving the API to paired devices on {}", running.url);
                state.running = Some(running);
                state.problem = None;
            }
            Err(problem) => {
                log::warn!("remote access is on but can't start: {problem}");
                state.problem = Some(problem);
            }
        }
    }

    async fn start_tailnet_listener(&self, port: u16) -> Result<RunningTailnetListener, String> {
        let tailscale = TailscaleCommand::find().ok_or("Tailscale isn't installed")?;
        let tailnet_self = tailscale.get_tailnet_self().await?;
        let listener = TailnetListener::bind(tailscale, &tailnet_self, port).await?;

        let host = format!("{}:{port}", tailnet_self.dns_name);
        let access = ListenerAccess {
            allowed_hosts: vec![host.clone()],
            tokens: AccessTokens::PairedDevices(Arc::clone(&self.paired_devices)),
        };
        let app = tailnet_router(Arc::clone(&self.backend)).layer(middleware::from_fn_with_state(
            access,
            require_listener_access,
        ));
        let (stop_sender, stop_receiver) = oneshot::channel::<()>();
        tokio::spawn(async move {
            let stopped = async {
                let _ = stop_receiver.await;
            };
            if let Err(error) = axum::serve(listener, app)
                .with_graceful_shutdown(stopped)
                .await
            {
                log::error!("the tailnet listener stopped: {error}");
            }
        });
        Ok(RunningTailnetListener {
            url: format!("https://{host}"),
            stop_sender,
        })
    }

    fn status_of(&self, state: &RemoteAccessState) -> RemoteAccessStatus {
        RemoteAccessStatus {
            enabled: state.settings.enabled,
            url: state.running.as_ref().map(|running| running.url.clone()),
            problem: state.problem.clone(),
            paired_devices: self.paired_devices.list(),
        }
    }
}

fn stop_listener(state: &mut RemoteAccessState) {
    if let Some(running) = state.running.take() {
        // The serving task may already be gone, which is a stop too.
        let _ = running.stop_sender.send(());
        log::info!("stopped serving the API to paired devices");
    }
}

fn read_settings(path: &std::path::Path) -> Result<RemoteAccessSettings, String> {
    match fs::read_to_string(path) {
        Ok(text) => {
            serde_json::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(RemoteAccessSettings::default()),
        Err(error) => Err(format!("{}: {error}", path.display())),
    }
}

fn write_settings(path: &std::path::Path, settings: &RemoteAccessSettings) -> Result<(), String> {
    let text = serde_json::to_string_pretty(settings).map_err(|error| error.to_string())?;
    fs::write(path, text).map_err(|error| format!("{}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_in(directory: &tempfile::TempDir) -> RemoteAccess {
        RemoteAccess::open(directory.path(), Arc::new(Backend::new())).unwrap()
    }

    #[tokio::test]
    async fn remote_access_starts_off_with_no_devices() {
        let directory = tempfile::tempdir().unwrap();

        let status = open_in(&directory).get_status().await;

        assert!(!status.enabled);
        assert_eq!(status.url, None);
        assert_eq!(status.problem, None);
        assert!(status.paired_devices.is_empty());
    }

    #[tokio::test]
    async fn turning_remote_access_off_is_remembered() {
        let directory = tempfile::tempdir().unwrap();
        open_in(&directory).set_enabled(false).await.unwrap();

        let settings = read_settings(&directory.path().join("remote_access.json")).unwrap();

        assert_eq!(settings, RemoteAccessSettings::default());
    }

    #[tokio::test]
    async fn pairing_needs_the_listener_running() {
        let directory = tempfile::tempdir().unwrap();

        let error = open_in(&directory)
            .create_paired_device("Phone")
            .await
            .unwrap_err();

        assert!(error.contains("turn on remote access"), "{error}");
        assert!(
            open_in(&directory)
                .get_status()
                .await
                .paired_devices
                .is_empty()
        );
    }
}
