//! The server's settings, read from the environment at start: where it
//! listens, where the built pages are, whether it is behind a proxy, how
//! many trial users it holds, its address as people reach it, its name
//! for the apps and the other servers it lists; and, one module each,
//! its database, mailer, buckets, Stripe and App Store.

pub mod app_store;
pub mod database;
pub mod mailer;
pub mod storage;
pub mod stripe;

use std::env;
use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;

use app_core::servers::{ListedServers, ServerDescription, ServerName};
use app_core::services::mailer::mails::APP_NAME;

const DEFAULT_HOST: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);
const DEFAULT_PORT: u16 = 4000;
const DEFAULT_SERVER_NAME: &str = "default";

/// The address to listen on: `HOST`, `0.0.0.0` in a container so its
/// proxy reaches the server, else only this machine.
pub fn host() -> Result<IpAddr, String> {
    match env::var("HOST") {
        Ok(value) => value
            .parse()
            .map_err(|_| format!("HOST must be an IP address, got '{value}'")),
        Err(_) => Ok(DEFAULT_HOST),
    }
}

/// The port to listen on: `PORT`, else 4000.
pub fn port() -> Result<u16, String> {
    match env::var("PORT") {
        Ok(value) => value
            .parse()
            .map_err(|_| format!("PORT must be a number, got '{value}'")),
        Err(_) => Ok(DEFAULT_PORT),
    }
}

/// The built Svelte pages: `SVELTE_DIST`, else `../svelte/dist` beside this crate.
pub fn svelte_dist() -> PathBuf {
    env::var_os("SVELTE_DIST")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../svelte/dist"))
}

/// The built ops pages: `SVELTE_OPS_DIST`, else `../svelte/dist-ops`
/// beside this crate.
pub fn svelte_ops_dist() -> PathBuf {
    env::var_os("SVELTE_OPS_DIST")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../svelte/dist-ops"))
}

/// Whether the client IP is the reverse proxy's `X-Forwarded-For`
/// header, `TRUST_PROXY_HEADERS=1`; off unless the proxy is the only way
/// in, see `rate_limits`.
pub fn trust_proxy_headers() -> bool {
    matches!(
        env::var("TRUST_PROXY_HEADERS").as_deref(),
        Ok("1") | Ok("true") | Ok("yes")
    )
}

/// How many trial users the server holds at once, `TRIAL_USERS_MAX`; 0,
/// the default, turns trials off, see `app_core::accounts::trial_users`.
#[derive(Clone, Copy)]
pub struct TrialUsersMax(pub i64);

impl TrialUsersMax {
    /// `TRIAL_USERS_MAX`; unset, 0.
    pub fn open() -> Result<TrialUsersMax, String> {
        match env::var("TRIAL_USERS_MAX") {
            Ok(max) => max
                .parse()
                .map(TrialUsersMax)
                .map_err(|_| format!("TRIAL_USERS_MAX must be a number, not '{max}'")),
            Err(_) => Ok(TrialUsersMax(0)),
        }
    }
}

/// The server's address as people reach it, with no trailing slash,
/// `SERVER_URL`, `http://localhost:4000` when unset. Stripe's return
/// links start with it.
#[derive(Clone)]
pub struct ServerUrl(pub String);

impl ServerUrl {
    pub fn open() -> Result<ServerUrl, String> {
        let url = env::var("SERVER_URL").unwrap_or_else(|_| "http://localhost:4000".to_string());
        let url = url.trim().trim_end_matches('/').to_string();
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(format!(
                "SERVER_URL must start with http:// or https://, got '{url}'"
            ));
        }
        Ok(ServerUrl(url))
    }
}

/// What this server says it is, see `app_core::servers`: `SERVER_NAME`,
/// a slug, `default` when unset, and `SERVER_DISPLAY_NAME`, the app's
/// name when unset.
pub fn server_description() -> Result<ServerDescription, String> {
    let name = env::var("SERVER_NAME").unwrap_or_else(|_| DEFAULT_SERVER_NAME.to_string());
    let name = ServerName::parse(name.trim()).map_err(|error| format!("SERVER_NAME: {error}"))?;
    let display_name = env::var("SERVER_DISPLAY_NAME")
        .ok()
        .map(|display_name| display_name.trim().to_string())
        .filter(|display_name| !display_name.is_empty())
        .unwrap_or_else(|| APP_NAME.to_string());
    Ok(ServerDescription::current(name, &display_name))
}

/// The other servers this one lists, `LISTED_SERVERS`, entries of a name
/// and an origin, `acme=https://acme.example,beta=https://beta.example`;
/// none when unset. A malformed entry stops the server at start.
pub fn listed_servers() -> Result<ListedServers, String> {
    let entries = env::var("LISTED_SERVERS").unwrap_or_default();
    ListedServers::parse(&entries).map_err(|error| format!("LISTED_SERVERS: {error}"))
}
