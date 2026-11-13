//! The pairing code a phone is given: the tailnet URL and the device's token
//! in one string to copy and paste, `cr1-<base64url of JSON>`.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};

const PREFIX: &str = "cr1-";

/// Where a paired device reaches the server, and its token.
#[derive(Clone, Debug)]
pub struct PairingCode {
    /// Such as `https://my-mac.tail1234.ts.net:7443`.
    pub url: String,
    pub token: SecretString,
}

#[derive(Serialize, Deserialize)]
struct PairingCodeJson {
    url: String,
    token: String,
}

impl PairingCode {
    /// The code as text to copy.
    pub fn encode(&self) -> String {
        let json = PairingCodeJson {
            url: self.url.clone(),
            token: self.token.expose_secret().to_string(),
        };
        // Serializing two strings can't fail.
        let bytes = serde_json::to_vec(&json).unwrap_or_default();
        format!("{PREFIX}{}", URL_SAFE_NO_PAD.encode(bytes))
    }

    /// Reads a pasted code, ignoring whitespace around it.
    pub fn decode(code: &str) -> Result<Self, String> {
        let not_a_code = || "that isn't a Code Reviewer pairing code".to_string();
        let encoded = code.trim().strip_prefix(PREFIX).ok_or_else(not_a_code)?;
        let bytes = URL_SAFE_NO_PAD.decode(encoded).map_err(|_| not_a_code())?;
        let json: PairingCodeJson = serde_json::from_slice(&bytes).map_err(|_| not_a_code())?;
        if !json.url.starts_with("https://") {
            return Err("the pairing code's URL isn't HTTPS".to_string());
        }
        Ok(Self {
            url: json.url,
            token: SecretString::from(json.token),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_code_decodes_to_what_was_encoded() {
        let code = PairingCode {
            url: "https://my-mac.tail1234.ts.net:7443".to_string(),
            token: SecretString::from("abc123"),
        };

        let decoded = PairingCode::decode(&format!("  {}\n", code.encode())).unwrap();

        assert_eq!(decoded.url, code.url);
        assert_eq!(decoded.token.expose_secret(), "abc123");
    }

    #[test]
    fn other_text_is_not_a_code() {
        assert!(PairingCode::decode("hello").is_err());
        assert!(PairingCode::decode("cr1-!!!").is_err());
        assert!(PairingCode::decode("cr1-e30").is_err());
    }

    #[test]
    fn a_code_must_point_at_https() {
        let code = PairingCode {
            url: "http://my-mac.tail1234.ts.net:7443".to_string(),
            token: SecretString::from("abc123"),
        };

        assert!(PairingCode::decode(&code.encode()).is_err());
    }
}
