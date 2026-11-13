//! The listener for paired devices, on this computer's Tailscale address,
//! serving HTTPS with Tailscale's certificate. Before a connection's TLS
//! handshake, its address must be a tailnet one and `tailscale whois` must
//! say the device belongs to this computer's owner: the listening address
//! alone isn't trusted to keep other networks out.

use std::collections::HashMap;
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::serve::Listener;
use rustls_pki_types::pem::PemObject;
use rustls_pki_types::{CertificateDer, PrivateKeyDer};
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::TlsAcceptor;
use tokio_rustls::rustls::ServerConfig as TlsConfig;
use tokio_rustls::server::TlsStream;

use crate::tailscale::{TailnetSelf, TailscaleCommand, is_tailnet_address};

/// How long a device `tailscale whois` approved stays approved without asking again.
const APPROVED_PEER_LIFETIME: Duration = Duration::from_secs(300);
/// How long a TLS handshake may take.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

/// Accepts TLS connections from the owner's tailnet devices only.
pub struct TailnetListener {
    tcp: TcpListener,
    acceptor: TlsAcceptor,
    tailscale: TailscaleCommand,
    owner_login: String,
    approved_peers: HashMap<IpAddr, Instant>,
}

impl TailnetListener {
    /// Listens on `port` of this computer's Tailscale IPv4 address, with a
    /// certificate from Tailscale for its DNS name.
    pub async fn bind(
        tailscale: TailscaleCommand,
        tailnet_self: &TailnetSelf,
        port: u16,
    ) -> Result<Self, String> {
        let pem = tailscale.create_certificate(&tailnet_self.dns_name).await?;
        let tls_config = create_tls_config(&pem)?;
        let address = SocketAddr::new(tailnet_self.ipv4_address, port);
        let tcp = TcpListener::bind(address)
            .await
            .map_err(|error| format!("couldn't listen on {address}: {error}"))?;
        Ok(Self {
            tcp,
            acceptor: TlsAcceptor::from(Arc::new(tls_config)),
            tailscale,
            owner_login: tailnet_self.owner_login.clone(),
            approved_peers: HashMap::new(),
        })
    }

    async fn check_peer(&mut self, address: IpAddr) -> Result<(), String> {
        if !is_tailnet_address(address) {
            return Err(format!("{address} isn't a tailnet address"));
        }
        if let Some(approved_at) = self.approved_peers.get(&address)
            && approved_at.elapsed() < APPROVED_PEER_LIFETIME
        {
            return Ok(());
        }
        let peer = self.tailscale.get_tailnet_peer(address).await?;
        if peer.owner_login != self.owner_login {
            self.approved_peers.remove(&address);
            return Err(format!(
                "{} belongs to {}, not this computer's owner",
                peer.device_name, peer.owner_login
            ));
        }
        self.approved_peers.insert(address, Instant::now());
        Ok(())
    }
}

impl Listener for TailnetListener {
    type Io = TlsStream<TcpStream>;
    type Addr = SocketAddr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        loop {
            let (stream, address) = match self.tcp.accept().await {
                Ok(accepted) => accepted,
                Err(error) => {
                    log::warn!("tailnet listener: couldn't accept a connection: {error}");
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                }
            };
            if let Err(reason) = self.check_peer(address.ip()).await {
                log::warn!("tailnet listener: refused {address}: {reason}");
                continue;
            }
            match tokio::time::timeout(HANDSHAKE_TIMEOUT, self.acceptor.accept(stream)).await {
                Ok(Ok(tls_stream)) => return (tls_stream, address),
                Ok(Err(error)) => {
                    log::warn!("tailnet listener: TLS with {address} failed: {error}")
                }
                Err(_) => log::warn!("tailnet listener: TLS with {address} took too long"),
            }
        }
    }

    fn local_addr(&self) -> io::Result<Self::Addr> {
        self.tcp.local_addr()
    }
}

/// A TLS configuration from Tailscale's PEM output: the certificate chain
/// and the private key, in either order.
fn create_tls_config(pem: &[u8]) -> Result<TlsConfig, String> {
    let certificates: Vec<CertificateDer<'static>> = CertificateDer::pem_slice_iter(pem)
        .collect::<Result<_, _>>()
        .map_err(|error| format!("couldn't read Tailscale's certificate: {error}"))?;
    if certificates.is_empty() {
        return Err("Tailscale wrote no certificate".to_string());
    }
    let private_key = PrivateKeyDer::from_pem_slice(pem)
        .map_err(|error| format!("couldn't read Tailscale's private key: {error}"))?;
    let provider = Arc::new(tokio_rustls::rustls::crypto::ring::default_provider());
    let mut config = TlsConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|error| error.to_string())?
        .with_no_client_auth()
        .with_single_cert(certificates, private_key)
        .map_err(|error| format!("Tailscale's certificate and key don't fit: {error}"))?;
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok(config)
}
