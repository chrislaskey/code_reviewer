//! Starting the server: the backend, a listener on localhost serving the
//! API to anyone with the token, and the tailnet listener for paired devices
//! when remote access is on. The desktop app calls [`start`] at
//! launch; a standalone server binary would call the same function.

use std::io;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::middleware;
use secrecy::SecretString;
use tokio::net::TcpListener;
use tokio::sync::oneshot;

use crate::access::{AccessTokens, ListenerAccess, require_listener_access};
use crate::backend::Backend;
use crate::http_api::localhost_router;
use crate::remote_access::RemoteAccess;
use crate::tokens::read_or_create_token_file;

/// The file in the data directory holding the localhost listener's token.
const DESKTOP_TOKEN_FILE_NAME: &str = "desktop_token";

/// Where the server keeps its files and what it listens on.
#[derive(Clone, Debug)]
pub struct ServerConfig {
    /// Holds the token files, created readable only by the user.
    pub data_directory: PathBuf,
    /// Must be a loopback address. Port 0 picks a free port.
    pub localhost_address: SocketAddr,
}

impl ServerConfig {
    /// Listens on a free port of `127.0.0.1`.
    pub fn new(data_directory: PathBuf) -> Self {
        Self {
            data_directory,
            localhost_address: SocketAddr::from(([127, 0, 0, 1], 0)),
        }
    }
}

/// Why the server couldn't start.
#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error("{path}: {source}")]
    Io { path: PathBuf, source: io::Error },
    #[error("{0} doesn't hold a token; delete it to create a new one")]
    MalformedTokenFile(PathBuf),
    #[error("couldn't create a random token: {0}")]
    Random(String),
    #[error(
        "{0} isn't a loopback address; the localhost listener must not be reachable from other machines"
    )]
    NotLoopback(SocketAddr),
    #[error("couldn't listen on {address}: {source}")]
    Listen {
        address: SocketAddr,
        source: io::Error,
    },
    #[error("couldn't read the remote access settings: {0}")]
    RemoteAccess(String),
}

impl ServerError {
    pub(crate) fn io(path: &Path, source: io::Error) -> Self {
        Self::Io {
            path: path.to_path_buf(),
            source,
        }
    }
}

/// A started server. It stops when this is dropped or [`shutdown`](Self::shutdown) is called.
pub struct RunningServer {
    localhost_address: SocketAddr,
    desktop_token: SecretString,
    stop_sender: Option<oneshot::Sender<()>>,
    // Owns the tailnet listener, which stops when this is dropped.
    _remote_access: Arc<RemoteAccess>,
}

impl RunningServer {
    /// The localhost listener's URL, such as `http://127.0.0.1:52341`.
    pub fn localhost_url(&self) -> String {
        format!("http://{}", self.localhost_address)
    }

    /// The token the localhost listener requires.
    pub fn desktop_token(&self) -> &SecretString {
        &self.desktop_token
    }

    /// Stops accepting connections; requests in flight finish.
    pub fn shutdown(mut self) {
        self.send_stop();
    }

    fn send_stop(&mut self) {
        if let Some(sender) = self.stop_sender.take() {
            // The serving task may already be gone, which is a stop too.
            let _ = sender.send(());
        }
    }
}

impl Drop for RunningServer {
    fn drop(&mut self) {
        self.send_stop();
    }
}

/// Creates the backend and starts the localhost listener on the current
/// Tokio runtime, creating the token file the first time.
pub async fn start(config: ServerConfig) -> Result<RunningServer, ServerError> {
    if !config.localhost_address.ip().is_loopback() {
        return Err(ServerError::NotLoopback(config.localhost_address));
    }
    let desktop_token =
        read_or_create_token_file(&config.data_directory.join(DESKTOP_TOKEN_FILE_NAME))?;

    let listener = TcpListener::bind(config.localhost_address)
        .await
        .map_err(|source| ServerError::Listen {
            address: config.localhost_address,
            source,
        })?;
    let localhost_address = listener
        .local_addr()
        .map_err(|source| ServerError::Listen {
            address: config.localhost_address,
            source,
        })?;

    let access = ListenerAccess {
        allowed_hosts: allowed_hosts_for(localhost_address),
        tokens: AccessTokens::One(desktop_token.clone()),
    };
    let backend = Arc::new(Backend::new());
    let remote_access = RemoteAccess::open(&config.data_directory, Arc::clone(&backend))
        .map(Arc::new)
        .map_err(ServerError::RemoteAccess)?;
    let app = localhost_router(backend, Arc::clone(&remote_access)).layer(
        middleware::from_fn_with_state(access, require_listener_access),
    );

    let (stop_sender, stop_receiver) = oneshot::channel::<()>();
    tokio::spawn(async move {
        let stopped = async {
            // A dropped sender is a stop as well.
            let _ = stop_receiver.await;
        };
        if let Err(error) = axum::serve(listener, app)
            .with_graceful_shutdown(stopped)
            .await
        {
            log::error!("the localhost listener stopped: {error}");
        }
    });
    log::info!("serving the API on http://{localhost_address}");

    // Starts the tailnet listener when remote access is on, without holding
    // up the launch: Tailscale's certificate can take seconds.
    let remote_access_to_start = Arc::clone(&remote_access);
    tokio::spawn(async move {
        remote_access_to_start.get_status().await;
    });

    Ok(RunningServer {
        localhost_address,
        desktop_token,
        stop_sender: Some(stop_sender),
        _remote_access: remote_access,
    })
}

/// The `Host` values a client of the localhost listener sends.
fn allowed_hosts_for(address: SocketAddr) -> Vec<String> {
    let port = address.port();
    vec![address.to_string(), format!("localhost:{port}")]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_non_loopback_address_is_refused() {
        let directory = tempfile::tempdir().unwrap();
        let config = ServerConfig {
            data_directory: directory.path().to_path_buf(),
            localhost_address: SocketAddr::from(([0, 0, 0, 0], 0)),
        };

        let result = start(config).await;

        assert!(matches!(result, Err(ServerError::NotLoopback(_))));
    }

    #[tokio::test]
    async fn a_started_server_listens_on_a_free_localhost_port() {
        let directory = tempfile::tempdir().unwrap();

        let server = start(ServerConfig::new(directory.path().to_path_buf()))
            .await
            .unwrap();

        assert!(server.localhost_url().starts_with("http://127.0.0.1:"));
        assert!(!server.localhost_url().ends_with(":0"));
        assert!(directory.path().join(DESKTOP_TOKEN_FILE_NAME).exists());
    }

    #[test]
    fn the_allowed_hosts_are_the_address_and_localhost_on_its_port() {
        let hosts = allowed_hosts_for(SocketAddr::from(([127, 0, 0, 1], 4100)));

        assert_eq!(hosts, vec!["127.0.0.1:4100", "localhost:4100"]);
    }
}
