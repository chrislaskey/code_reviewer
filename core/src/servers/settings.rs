//! What a server offers, beyond what it is: one small document of keys
//! and values, `ServerSettings`, see "Server settings" in
//! `docs/design/multi-server.md`. Mostly keys that turn on code the app
//! already has, `features`, see `app::features`; never code or markup.
//! The server holds the truth and refuses what is off; a device keeps a
//! copy only to decide what it offers. A device reads the document
//! leniently, an unknown key ignored and a missing one defaulted, so a
//! new setting needs no protocol version; the server reads its own
//! strictly, so a typo stops it at start.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::app::features::Features;

/// The header every sync response carries the server's settings version
/// in, so a device whose copy differs fetches the document again.
pub const SETTINGS_VERSION_HEADER: &str = "x-settings-version";

/// A server's settings, each key with the value it has when left out.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerSettings {
    pub features: Features,
    pub communities: Communities,
}

/// How many communities the pages show a person: `Multiple`, every
/// community, the switcher and the communities pages; `Single`, only
/// the one they work in, with none of those, for an app that is not
/// many groups' at once. The communities are made the same either way.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Communities {
    #[default]
    Multiple,
    Single,
}

impl ServerSettings {
    /// The server's own settings, from its config: JSON whose every key
    /// is known; blank, the defaults.
    pub fn parse_strict(json: &str) -> Result<ServerSettings, SettingsError> {
        if json.trim().is_empty() {
            return Ok(ServerSettings::default());
        }
        let given: Value = serde_json::from_str(json)
            .map_err(|error| SettingsError::Malformed(error.to_string()))?;
        let settings: ServerSettings = serde_json::from_value(given.clone())
            .map_err(|error| SettingsError::Malformed(error.to_string()))?;
        let known = serde_json::to_value(&settings)
            .map_err(|error| SettingsError::Malformed(error.to_string()))?;
        let mut unknown = Vec::new();
        unknown_keys(&given, &known, "", &mut unknown);
        if !unknown.is_empty() {
            return Err(SettingsError::UnknownKeys(unknown.join(", ")));
        }
        if settings.features.accounts_sign_up && !settings.features.accounts_sign_in_code {
            return Err(SettingsError::SignUpNeedsSignInCode);
        }
        Ok(settings)
    }

    /// The hash of the settings, which changes whenever one does.
    pub fn version(&self) -> String {
        let json = serde_json::to_vec(self).unwrap_or_default();
        format!("{:x}", Sha256::digest(json))
    }
}

/// The settings as a server answers them, with their version, read
/// together so a device never keeps one with the other's.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingsDocument {
    pub settings_version: String,
    pub settings: ServerSettings,
}

impl SettingsDocument {
    pub fn of(settings: ServerSettings) -> SettingsDocument {
        SettingsDocument {
            settings_version: settings.version(),
            settings,
        }
    }
}

/// The keys of `given` that `known` lacks, by their dotted paths.
fn unknown_keys(given: &Value, known: &Value, path: &str, unknown: &mut Vec<String>) {
    let (Value::Object(given), Value::Object(known)) = (given, known) else {
        return;
    };
    for (key, value) in given {
        let key_path = if path.is_empty() {
            key.clone()
        } else {
            format!("{path}.{key}")
        };
        match known.get(key) {
            Some(known_value) => unknown_keys(value, known_value, &key_path, unknown),
            None => unknown.push(key_path),
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SettingsError {
    #[error("the server's settings are not valid: {0}")]
    Malformed(String),
    #[error("the server's settings have keys this server does not know: {0}")]
    UnknownKeys(String),
    #[error(
        "the server's settings take sign-ups but no sign-in codes, and only a code proves a new email; set features.accounts_sign_up to false too"
    )]
    SignUpNeedsSignInCode,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn servers_settings_a_device_ignores_unknown_keys_and_defaults_missing_ones() {
        let settings: ServerSettings =
            serde_json::from_str(r#"{"features": {"forum_posting": true}, "colors": {}}"#)
                .expect("read");
        assert_eq!(settings, ServerSettings::default());
        assert!(settings.features.accounts_sign_up);
        let settings: ServerSettings =
            serde_json::from_str(r#"{"features": {"accounts_sign_up": false}}"#).expect("read");
        assert!(!settings.features.accounts_sign_up);
    }

    #[test]
    fn servers_settings_the_server_refuses_unknown_keys_and_wrong_types() {
        assert_eq!(
            ServerSettings::parse_strict(" "),
            Ok(ServerSettings::default())
        );
        let closed = ServerSettings::parse_strict(r#"{"features": {"accounts_sign_up": false}}"#)
            .expect("parsed");
        assert!(!closed.features.accounts_sign_up);
        assert_eq!(
            ServerSettings::parse_strict(r#"{"features": {"accounts_signup": false}, "x": 1}"#),
            Err(SettingsError::UnknownKeys(
                "features.accounts_signup, x".to_string()
            ))
        );
        assert!(matches!(
            ServerSettings::parse_strict(r#"{"features": {"accounts_sign_up": "no"}}"#),
            Err(SettingsError::Malformed(_))
        ));
        assert!(matches!(
            ServerSettings::parse_strict("features"),
            Err(SettingsError::Malformed(_))
        ));
    }

    #[test]
    fn servers_settings_communities_are_multiple_unless_set_to_single() {
        assert_eq!(ServerSettings::default().communities, Communities::Multiple);
        let single = ServerSettings::parse_strict(r#"{"communities": "single"}"#).expect("parsed");
        assert_eq!(single.communities, Communities::Single);
        assert!(matches!(
            ServerSettings::parse_strict(r#"{"communities": "few"}"#),
            Err(SettingsError::Malformed(_))
        ));
    }

    #[test]
    fn servers_settings_the_server_refuses_sign_ups_without_sign_in_codes() {
        assert_eq!(
            ServerSettings::parse_strict(r#"{"features": {"accounts_sign_in_code": false}}"#),
            Err(SettingsError::SignUpNeedsSignInCode)
        );
        let password_only = ServerSettings::parse_strict(
            r#"{"features": {"accounts_sign_up": false, "accounts_sign_in_code": false}}"#,
        )
        .expect("parsed");
        assert!(!password_only.features.accounts_sign_in_code);
    }

    #[test]
    fn servers_settings_version_changes_with_a_setting_alone() {
        let defaults = ServerSettings::default();
        assert_eq!(defaults.version(), ServerSettings::default().version());
        let closed = ServerSettings {
            features: Features {
                accounts_sign_up: false,
                ..Features::default()
            },
            ..ServerSettings::default()
        };
        assert_ne!(defaults.version(), closed.version());
        assert_eq!(
            SettingsDocument::of(closed.clone()).settings_version,
            closed.version()
        );
    }
}
