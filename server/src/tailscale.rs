//! What the server asks of Tailscale, through its `tailscale` command, which
//! finds the running Tailscale on every platform (on macOS the app's local
//! API is a TCP port with a password only the command knows how to find).

use std::collections::HashMap;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;
use tokio::process::Command;

/// How long a `tailscale` command may take.
const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);

/// Where the macOS app keeps the command when it isn't on `PATH`.
const MACOS_APP_COMMAND: &str = "/Applications/Tailscale.app/Contents/MacOS/Tailscale";

/// This computer on the tailnet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TailnetSelf {
    /// Such as `my-mac.tail1234.ts.net`, without the trailing dot.
    pub dns_name: String,
    pub ipv4_address: IpAddr,
    /// The login of the user this computer belongs to.
    pub owner_login: String,
}

/// Who is at the other end of a tailnet connection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TailnetPeer {
    pub device_name: String,
    /// `tagged-devices` for a device owned by tags rather than a user.
    pub owner_login: String,
}

/// The `tailscale` command.
#[derive(Clone, Debug)]
pub struct TailscaleCommand {
    path: PathBuf,
}

impl TailscaleCommand {
    /// The command on `PATH`, else the macOS app's, or `None` when neither exists.
    pub fn find() -> Option<Self> {
        let on_path = std::env::var_os("PATH").and_then(|paths| {
            std::env::split_paths(&paths)
                .map(|directory| directory.join("tailscale"))
                .find(|candidate| candidate.is_file())
        });
        on_path
            .or_else(|| Some(PathBuf::from(MACOS_APP_COMMAND)).filter(|path| path.is_file()))
            .map(|path| Self { path })
    }

    /// This computer's name, address and owner, or an error saying why
    /// remote access can't work yet (Tailscale stopped, HTTPS off).
    pub async fn get_tailnet_self(&self) -> Result<TailnetSelf, String> {
        let output = self.run(&["status", "--json"]).await?;
        parse_status(&output)
    }

    /// The device and user behind a tailnet address.
    pub async fn get_tailnet_peer(&self, address: IpAddr) -> Result<TailnetPeer, String> {
        let output = self.run(&["whois", "--json", &address.to_string()]).await?;
        parse_whois(&output)
    }

    /// A TLS certificate chain and private key for `dns_name`, both PEM,
    /// written by Tailscale to standard output (the macOS app is sandboxed,
    /// so it may not write files elsewhere).
    pub async fn create_certificate(&self, dns_name: &str) -> Result<Vec<u8>, String> {
        let output = self
            .run_raw(&["cert", "--cert-file", "-", "--key-file", "-", dns_name])
            .await?;
        Ok(output)
    }

    async fn run(&self, arguments: &[&str]) -> Result<String, String> {
        let output = self.run_raw(arguments).await?;
        String::from_utf8(output).map_err(|error| format!("tailscale wrote invalid UTF-8: {error}"))
    }

    async fn run_raw(&self, arguments: &[&str]) -> Result<Vec<u8>, String> {
        let command = Command::new(&self.path)
            .args(arguments)
            .kill_on_drop(true)
            .output();
        let output = tokio::time::timeout(COMMAND_TIMEOUT, command)
            .await
            .map_err(|_| format!("tailscale {} took too long", arguments[0]))?
            .map_err(|error| format!("couldn't run {}: {error}", display_path(&self.path)))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "tailscale {} failed: {}",
                arguments[0],
                stderr.trim()
            ));
        }
        Ok(output.stdout)
    }
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct StatusJson {
    backend_state: String,
    #[serde(rename = "Self")]
    self_node: Option<StatusNodeJson>,
    #[serde(default)]
    user: HashMap<String, UserProfileJson>,
    #[serde(default)]
    cert_domains: Option<Vec<String>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct StatusNodeJson {
    #[serde(rename = "DNSName")]
    dns_name: String,
    #[serde(rename = "TailscaleIPs", default)]
    tailscale_ips: Vec<IpAddr>,
    #[serde(rename = "UserID")]
    user_id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct UserProfileJson {
    login_name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct WhoisJson {
    node: WhoisNodeJson,
    user_profile: UserProfileJson,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct WhoisNodeJson {
    name: String,
}

fn parse_status(output: &str) -> Result<TailnetSelf, String> {
    let status: StatusJson = serde_json::from_str(output)
        .map_err(|error| format!("couldn't read tailscale status: {error}"))?;
    if status.backend_state != "Running" {
        return Err(format!(
            "Tailscale isn't connected (it is {})",
            status.backend_state
        ));
    }
    let node = status
        .self_node
        .ok_or("tailscale status doesn't describe this computer")?;
    let dns_name = node.dns_name.trim_end_matches('.').to_string();
    let has_certificate_domain = status
        .cert_domains
        .unwrap_or_default()
        .iter()
        .any(|domain| domain == &dns_name);
    if !has_certificate_domain {
        return Err(
            "HTTPS certificates are off for this tailnet; turn them on in the Tailscale admin console (DNS page)"
                .to_string(),
        );
    }
    let ipv4_address = node
        .tailscale_ips
        .into_iter()
        .find(IpAddr::is_ipv4)
        .ok_or("this computer has no Tailscale IPv4 address")?;
    let owner_login = status
        .user
        .get(&node.user_id.to_string())
        .map(|profile| profile.login_name.clone())
        .ok_or("tailscale status doesn't name this computer's user")?;
    Ok(TailnetSelf {
        dns_name,
        ipv4_address,
        owner_login,
    })
}

fn parse_whois(output: &str) -> Result<TailnetPeer, String> {
    let whois: WhoisJson = serde_json::from_str(output)
        .map_err(|error| format!("couldn't read tailscale whois: {error}"))?;
    Ok(TailnetPeer {
        device_name: whois.node.name.trim_end_matches('.').to_string(),
        owner_login: whois.user_profile.login_name,
    })
}

/// Whether `address` is in Tailscale's ranges: `100.64.0.0/10` or `fd7a:115c:a1e0::/48`.
pub fn is_tailnet_address(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(v4) => {
            let [first, second, ..] = v4.octets();
            first == 100 && (64..128).contains(&second)
        }
        IpAddr::V6(v6) => {
            let segments = v6.segments();
            segments[0] == 0xfd7a && segments[1] == 0x115c && segments[2] == 0xa1e0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUNNING_STATUS: &str = r#"{
        "BackendState": "Running",
        "Self": {
            "DNSName": "my-mac.tail1234.ts.net.",
            "TailscaleIPs": ["100.101.102.103", "fd7a:115c:a1e0::1"],
            "UserID": 42
        },
        "User": { "42": { "ID": 42, "LoginName": "me@example.com" } },
        "CertDomains": ["my-mac.tail1234.ts.net"]
    }"#;

    #[test]
    fn a_running_status_names_this_computer_and_its_owner() {
        let tailnet_self = parse_status(RUNNING_STATUS).unwrap();

        assert_eq!(
            tailnet_self,
            TailnetSelf {
                dns_name: "my-mac.tail1234.ts.net".to_string(),
                ipv4_address: "100.101.102.103".parse().unwrap(),
                owner_login: "me@example.com".to_string(),
            }
        );
    }

    #[test]
    fn a_stopped_tailscale_is_an_error_saying_so() {
        let error = parse_status(r#"{"BackendState": "Stopped", "Self": null}"#).unwrap_err();

        assert_eq!(error, "Tailscale isn't connected (it is Stopped)");
    }

    #[test]
    fn a_tailnet_without_https_certificates_is_an_error_saying_how_to_fix_it() {
        let status = RUNNING_STATUS.replace(r#"["my-mac.tail1234.ts.net"]"#, "null");

        let error = parse_status(&status).unwrap_err();

        assert!(error.contains("HTTPS certificates are off"), "{error}");
    }

    #[test]
    fn whois_names_the_device_and_its_owner() {
        let output = r#"{
            "Node": { "ID": 7, "Name": "my-phone.tail1234.ts.net.", "User": 42 },
            "UserProfile": { "ID": 42, "LoginName": "me@example.com", "DisplayName": "Me" },
            "CapMap": null
        }"#;

        let peer = parse_whois(output).unwrap();

        assert_eq!(
            peer,
            TailnetPeer {
                device_name: "my-phone.tail1234.ts.net".to_string(),
                owner_login: "me@example.com".to_string(),
            }
        );
    }

    #[test]
    fn tailnet_addresses_are_tailscales_ranges_only() {
        assert!(is_tailnet_address("100.64.0.1".parse().unwrap()));
        assert!(is_tailnet_address("100.127.255.254".parse().unwrap()));
        assert!(is_tailnet_address("fd7a:115c:a1e0::1".parse().unwrap()));
        assert!(!is_tailnet_address("100.63.255.255".parse().unwrap()));
        assert!(!is_tailnet_address("100.128.0.1".parse().unwrap()));
        assert!(!is_tailnet_address("192.168.1.10".parse().unwrap()));
        assert!(!is_tailnet_address("127.0.0.1".parse().unwrap()));
        assert!(!is_tailnet_address("fd7a:115c:a1e1::1".parse().unwrap()));
    }
}
