//! Sends session notifications to iPhones through Apple Push Notification
//! service's sandbox, which reaches development builds of the app. Needs the
//! APNs key and its ids in the data directory (see
//! `docs/guides/apple-push-notifications.md`); without them push is off.
//! See decision 30 in `docs/decisions.md`.

use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use jsonwebtoken::{Algorithm, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::app::sessions::notifications::SessionNotification;
use crate::app::sessions::private_file::read_json_or_default;

const SETTINGS_FILE_NAME: &str = "apple_push_notifications.json";
const KEY_FILE_NAME: &str = "apple_push_notifications_key.p8";
const SANDBOX_URL: &str = "https://api.sandbox.push.apple.com";
/// The iOS app's bundle id.
const APP_BUNDLE_ID: &str = "com.chrislaskey.codereviewer";
/// Apple refuses a signed token over an hour old, and one made more often
/// than every 20 minutes.
const SIGNED_TOKEN_LIFETIME: Duration = Duration::from_secs(50 * 60);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
/// The key in a push's data holding its session's name, for the app to open.
const SESSION_NAME_KEY: &str = "session_name";

/// The ids that go with the APNs key, from `apple_push_notifications.json`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
struct ApplePushSettings {
    key_id: String,
    team_id: String,
}

/// Why a push wasn't sent.
#[derive(Debug, thiserror::Error)]
pub enum ApplePushError {
    /// The device token is no longer any use, so it should be forgotten.
    #[error("Apple no longer accepts the device token: {0}")]
    DeviceTokenNotValid(String),
    #[error("sending a push notification failed: {0}")]
    Failed(String),
}

/// Signs and sends pushes with one APNs key.
pub struct ApplePushSender {
    client: reqwest::Client,
    key: EncodingKey,
    settings: ApplePushSettings,
    signed_token: Mutex<Option<SignedToken>>,
}

struct SignedToken {
    token: String,
    made_at: Instant,
}

#[derive(Serialize)]
struct SignedTokenClaims<'a> {
    iss: &'a str,
    iat: u64,
}

impl ApplePushSender {
    /// A sender for the key in `data_directory`, or `None` when there is no
    /// key or settings file.
    pub fn open(data_directory: &Path) -> Result<Option<Self>, String> {
        let settings_path = data_directory.join(SETTINGS_FILE_NAME);
        let key_path = data_directory.join(KEY_FILE_NAME);
        if !settings_path.exists() || !key_path.exists() {
            return Ok(None);
        }
        let settings: ApplePushSettings = read_json_or_default(&settings_path)?;
        let key_pem =
            std::fs::read(&key_path).map_err(|error| format!("{}: {error}", key_path.display()))?;
        let key = EncodingKey::from_ec_pem(&key_pem)
            .map_err(|error| format!("{}: {error}", key_path.display()))?;
        let client = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|error| error.to_string())?;
        Ok(Some(Self {
            client,
            key,
            settings,
            signed_token: Mutex::new(None),
        }))
    }

    /// Sends `notification` to the phone with `device_token`.
    pub async fn send(
        &self,
        device_token: &str,
        notification: &SessionNotification,
    ) -> Result<(), ApplePushError> {
        let signed_token = self.signed_token()?;
        let response = self
            .client
            .post(format!("{SANDBOX_URL}/3/device/{device_token}"))
            .bearer_auth(signed_token)
            .header("apns-topic", APP_BUNDLE_ID)
            .header("apns-push-type", "alert")
            .header("apns-priority", "10")
            .json(&payload_of(notification))
            .send()
            .await
            .map_err(|error| ApplePushError::Failed(error.to_string()))?;
        let status = response.status().as_u16();
        if status == 200 {
            return Ok(());
        }
        let reason = response
            .json::<serde_json::Value>()
            .await
            .ok()
            .and_then(|body| body["reason"].as_str().map(str::to_string))
            .unwrap_or_else(|| format!("status {status}"));
        Err(error_of(status, reason))
    }

    /// The signed token Apple checks each push with, made again when old.
    fn signed_token(&self) -> Result<String, ApplePushError> {
        let mut signed_token = self
            .signed_token
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(signed) = signed_token.as_ref()
            && signed.made_at.elapsed() < SIGNED_TOKEN_LIFETIME
        {
            return Ok(signed.token.clone());
        }
        let mut header = Header::new(Algorithm::ES256);
        header.kid = Some(self.settings.key_id.clone());
        let claims = SignedTokenClaims {
            iss: &self.settings.team_id,
            iat: chrono::Utc::now().timestamp().unsigned_abs(),
        };
        let token = jsonwebtoken::encode(&header, &claims, &self.key)
            .map_err(|error| ApplePushError::Failed(format!("signing the request: {error}")))?;
        *signed_token = Some(SignedToken {
            token: token.clone(),
            made_at: Instant::now(),
        });
        Ok(token)
    }
}

fn payload_of(notification: &SessionNotification) -> serde_json::Value {
    json!({
        "aps": {
            "alert": { "title": notification.title, "body": notification.body },
            "sound": "default",
        },
        SESSION_NAME_KEY: notification.session_name,
    })
}

/// A device token is no longer valid when the app was deleted
/// (`Unregistered`) or it was made for the other APNs host (`BadDeviceToken`).
fn error_of(status: u16, reason: String) -> ApplePushError {
    match (status, reason.as_str()) {
        (410, _) | (400, "BadDeviceToken") => ApplePushError::DeviceTokenNotValid(reason),
        _ => ApplePushError::Failed(reason),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_is_off_without_the_key_and_its_settings() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(
            directory.path().join(SETTINGS_FILE_NAME),
            r#"{"key_id":"ABC","team_id":"DEF"}"#,
        )
        .unwrap();

        assert!(ApplePushSender::open(directory.path()).unwrap().is_none());
    }

    #[test]
    fn a_push_has_the_notifications_text_and_its_session() {
        let notification = SessionNotification {
            session_name: "cr-a".into(),
            title: "cr-a".into(),
            body: "Done".into(),
        };

        assert_eq!(
            payload_of(&notification),
            json!({
                "aps": { "alert": { "title": "cr-a", "body": "Done" }, "sound": "default" },
                "session_name": "cr-a",
            })
        );
    }

    #[test]
    fn only_an_unregistered_or_bad_device_token_is_not_valid() {
        assert!(matches!(
            error_of(410, "Unregistered".into()),
            ApplePushError::DeviceTokenNotValid(_)
        ));
        assert!(matches!(
            error_of(400, "BadDeviceToken".into()),
            ApplePushError::DeviceTokenNotValid(_)
        ));
        assert!(matches!(
            error_of(403, "InvalidProviderToken".into()),
            ApplePushError::Failed(_)
        ));
    }
}
