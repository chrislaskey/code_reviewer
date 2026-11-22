//! What a server says it is, for a device signing in to a server other
//! than its build's default, see `docs/design/multi-server.md`: every
//! server describes itself, `ServerDescription`, at
//! `/api/servers/current`, and the default server says whether it lists
//! another, `ListedServers`, from a static list in its config, answering
//! an entry only when both its name and its origin match. Trust rests on
//! the origin a device connects to, `origin_of`, never on what a server
//! says of itself. What a server offers is its settings, `settings`.

pub mod settings;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::Url;

use crate::app::identity::APP_IDENTIFIER;
use crate::servers::settings::ServerSettings;
use crate::sync::PROTOCOL_VERSION;

/// The name no server may have: `/api/servers/current` is the server
/// asked describing itself.
pub const RESERVED_NAME: &str = "current";

/// What a server says it is.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerDescription {
    /// The app the server runs, `app::identity::APP_IDENTIFIER`.
    pub app_identifier: String,
    /// The sync's version, `sync::PROTOCOL_VERSION`.
    pub protocol_version: u32,
    /// A slug, the name the default server lists it by.
    pub name: String,
    /// For display only.
    pub display_name: String,
    /// The version of its settings, see `settings`.
    pub settings_version: String,
}

impl ServerDescription {
    /// This server's description, with its name, display name and
    /// settings.
    pub fn current(
        name: ServerName,
        display_name: &str,
        settings: &ServerSettings,
    ) -> ServerDescription {
        ServerDescription {
            app_identifier: APP_IDENTIFIER.to_string(),
            protocol_version: PROTOCOL_VERSION,
            name: name.0,
            display_name: display_name.to_string(),
            settings_version: settings.version(),
        }
    }

    /// Whether a device of this app and protocol version can sync with
    /// the server; the reason when not.
    pub fn compatible(&self) -> Result<(), ServerError> {
        if self.app_identifier != APP_IDENTIFIER {
            return Err(ServerError::OtherApp {
                app_identifier: self.app_identifier.clone(),
            });
        }
        if self.protocol_version != PROTOCOL_VERSION {
            return Err(ServerError::OtherProtocolVersion {
                server: self.protocol_version,
                app: PROTOCOL_VERSION,
            });
        }
        Ok(())
    }
}

/// A server's name: a slug of lowercase letters, digits and hyphens,
/// never `current`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerName(String);

impl ServerName {
    pub fn parse(name: &str) -> Result<ServerName, ServerError> {
        let slug = !name.is_empty()
            && name.len() <= 63
            && name
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-');
        if !slug || name == RESERVED_NAME {
            return Err(ServerError::Name {
                name: name.to_string(),
            });
        }
        Ok(ServerName(name.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A server the default server lists, by its name and origin.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListedServer {
    pub name: String,
    pub url: String,
}

/// The servers the default server lists.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ListedServers(Vec<ListedServer>);

impl ListedServers {
    /// From entries of a name and an origin,
    /// `acme=https://acme.example,beta=https://beta.example`; blank, none.
    pub fn parse(entries: &str) -> Result<ListedServers, ServerError> {
        entries
            .split(',')
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .map(|entry| {
                let (name, url) = entry.split_once('=').ok_or_else(|| ServerError::Entry {
                    entry: entry.to_string(),
                })?;
                Ok(ListedServer {
                    name: ServerName::parse(name.trim())?.0,
                    url: origin_of(url.trim(), true)?,
                })
            })
            .collect::<Result<Vec<_>, _>>()
            .map(ListedServers)
    }

    /// The entry with both the name and the origin of the URL; none when
    /// either differs, or the URL is no origin.
    pub fn get(&self, name: &str, url: &str) -> Option<&ListedServer> {
        let origin = origin_of(url, true).ok()?;
        self.0
            .iter()
            .find(|listed| listed.name == name && listed.url == origin)
    }
}

/// A URL as the origin a device connects to: `https://host[:port]`, the
/// host lowercased, the port kept unless it is 443. Plain http only to
/// `localhost` or `127.0.0.1`, and only when `local_allowed`, for dev
/// builds. A path, a query, a fragment or a `user:password@` is refused,
/// never dropped.
pub fn origin_of(url: &str, local_allowed: bool) -> Result<String, ServerError> {
    let refused = |reason: &str| ServerError::Url {
        url: url.to_string(),
        reason: reason.to_string(),
    };
    let parsed = Url::parse(url.trim()).map_err(|error| refused(&error.to_string()))?;
    let host = parsed.host_str().ok_or_else(|| refused("it has no host"))?;
    let local = matches!(host, "localhost" | "127.0.0.1");
    match parsed.scheme() {
        "https" => {}
        "http" if local && local_allowed => {}
        "http" => return Err(refused("it must start with https://")),
        _ => return Err(refused("it must start with https://")),
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(refused("it must not hold a user or password"));
    }
    if parsed.path() != "/" || parsed.query().is_some() || parsed.fragment().is_some() {
        return Err(refused(
            "it must be the server's address alone, with no path",
        ));
    }
    Ok(match parsed.port() {
        Some(port) => format!("{}://{host}:{port}", parsed.scheme()),
        None => format!("{}://{host}", parsed.scheme()),
    })
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ServerError {
    #[error("{url} is not a server's address: {reason}")]
    Url { url: String, reason: String },
    #[error(
        "'{name}' is not a server's name: lowercase letters, digits and hyphens, never 'current'"
    )]
    Name { name: String },
    #[error("'{entry}' is not a listed server: a name, '=', and its address")]
    Entry { entry: String },
    #[error("That server runs another app, {app_identifier}")]
    OtherApp { app_identifier: String },
    #[error("{}", crate::sync::protocol_version_mismatch(*server, *app))]
    OtherProtocolVersion { server: u32, app: u32 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn servers_origin_of_normalizes_and_refuses_what_it_would_drop() {
        assert_eq!(
            origin_of("https://Acme.Example.com", false),
            Ok("https://acme.example.com".to_string())
        );
        assert_eq!(
            origin_of(" https://acme.example.com/ ", false),
            Ok("https://acme.example.com".to_string())
        );
        assert_eq!(
            origin_of("https://acme.example.com:443", false),
            Ok("https://acme.example.com".to_string())
        );
        assert_eq!(
            origin_of("https://mac.tail1234.ts.net:7443", false),
            Ok("https://mac.tail1234.ts.net:7443".to_string())
        );
        assert_eq!(
            origin_of("http://localhost:4700", true),
            Ok("http://localhost:4700".to_string())
        );
        for refused in [
            "http://localhost:4700",
            "http://acme.example.com",
            "ftp://acme.example.com",
            "https://acme.example.com/app",
            "https://acme.example.com/?x=1",
            "https://acme.example.com/#top",
            "https://alice:secret@acme.example.com",
            "acme.example.com",
            "",
        ] {
            assert!(origin_of(refused, false).is_err(), "{refused}");
        }
        assert!(
            origin_of("http://acme.example.com", true).is_err(),
            "plain http only to this machine"
        );
    }

    #[test]
    fn servers_name_is_a_slug_and_never_current() {
        assert!(ServerName::parse("acme-2").is_ok());
        for refused in ["", "Acme", "acme corp", "acme_corp", "current", "é"] {
            assert!(ServerName::parse(refused).is_err(), "{refused}");
        }
    }

    #[test]
    fn servers_listed_get_matches_the_name_and_the_origin_both() {
        let listed = ListedServers::parse(
            " acme=https://Acme.Example.com/ , beta=https://beta.example.com:8443,",
        )
        .expect("parsed");
        let acme = listed
            .get("acme", "https://acme.example.com")
            .expect("listed");
        assert_eq!(acme.url, "https://acme.example.com");
        assert!(
            listed
                .get("beta", "https://beta.example.com:8443")
                .is_some()
        );
        assert_eq!(
            listed.get("acme", "https://evil.example.com"),
            None,
            "a listed name at another origin"
        );
        assert_eq!(
            listed.get("evil", "https://acme.example.com"),
            None,
            "a listed origin under another name"
        );
        assert_eq!(listed.get("acme", "not a url"), None);
        assert_eq!(
            ListedServers::parse("").expect("none"),
            ListedServers::default()
        );
    }

    #[test]
    fn servers_listed_refuses_a_malformed_entry_or_the_reserved_name() {
        assert!(matches!(
            ListedServers::parse("acme"),
            Err(ServerError::Entry { .. })
        ));
        assert!(matches!(
            ListedServers::parse("current=https://acme.example.com"),
            Err(ServerError::Name { .. })
        ));
        assert!(matches!(
            ListedServers::parse("acme=https://acme.example.com/app"),
            Err(ServerError::Url { .. })
        ));
    }

    #[test]
    fn servers_description_is_compatible_with_this_app_and_version_alone() {
        let name = ServerName::parse("acme").expect("a name");
        let current = ServerDescription::current(name, "Acme", &ServerSettings::default());
        assert_eq!(current.compatible(), Ok(()));
        let other_app = ServerDescription {
            app_identifier: "com.example.other".to_string(),
            ..current.clone()
        };
        assert!(matches!(
            other_app.compatible(),
            Err(ServerError::OtherApp { .. })
        ));
        let newer = ServerDescription {
            protocol_version: PROTOCOL_VERSION + 1,
            ..current
        };
        assert!(matches!(
            newer.compatible(),
            Err(ServerError::OtherProtocolVersion { .. })
        ));
    }
}
