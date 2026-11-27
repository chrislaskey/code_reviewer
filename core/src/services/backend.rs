//! The remote backend: a server in another language that owns the web
//! server's database alone, such as `core_elixir/`, see
//! `docs/design/backend-seam.md`. Core calls it over the backend
//! protocol, `docs/design/backend-protocol.md`: a JSON `POST` to
//! `/backend/<resource>/<action>`, with the service token. Only the web
//! server has one. A device never does.

use std::collections::BTreeMap;
use std::time::Duration;

use secrecy::{ExposeSecret, SecretString};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use thiserror::Error;

use crate::helpers::field_errors::FieldErrors;

/// How long one call may take.
const TIMEOUT: Duration = Duration::from_secs(30);

/// A client for the remote backend at its URL.
#[derive(Clone, Debug)]
pub struct RemoteBackend {
    client: reqwest::Client,
    base_url: String,
    service_token: SecretString,
}

#[derive(Debug, Error)]
pub enum BackendError {
    /// The backend answered with one of the protocol's error codes.
    #[error("{message}")]
    Answered {
        status: u16,
        code: String,
        message: String,
        fields: Option<FieldErrors>,
    },
    /// The backend was not reached, or did not answer in time.
    #[error("the backend could not be reached: {0}")]
    Unreachable(String),
    /// The backend answered something the protocol does not say.
    #[error("the backend's answer was not understood: {0}")]
    Malformed(String),
}

impl BackendError {
    /// The protocol's error code, when the backend answered one.
    pub fn code(&self) -> Option<&str> {
        match self {
            BackendError::Answered { code, .. } => Some(code),
            _ => None,
        }
    }
}

impl RemoteBackend {
    /// A client for the backend at `base_url`, which sends the service token.
    pub fn new(base_url: &str, service_token: SecretString) -> Result<RemoteBackend, BackendError> {
        let client = reqwest::Client::builder()
            .timeout(TIMEOUT)
            .build()
            .map_err(|error| BackendError::Unreachable(error.to_string()))?;
        Ok(RemoteBackend {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            service_token,
        })
    }

    /// Calls one operation, `"users/get_by_token"` say, with its request,
    /// and reads its answer.
    pub async fn call<Request, Answer>(
        &self,
        operation: &str,
        request: &Request,
    ) -> Result<Answer, BackendError>
    where
        Request: Serialize + ?Sized,
        Answer: DeserializeOwned,
    {
        let response = self
            .client
            .post(format!("{}/backend/{operation}", self.base_url))
            .bearer_auth(self.service_token.expose_secret())
            .json(request)
            .send()
            .await
            .map_err(|error| BackendError::Unreachable(error.to_string()))?;
        let status = response.status().as_u16();
        let body = response
            .bytes()
            .await
            .map_err(|error| BackendError::Unreachable(error.to_string()))?;
        if (200..300).contains(&status) {
            return serde_json::from_slice(&body)
                .map_err(|error| BackendError::Malformed(format!("{operation}: {error}")));
        }
        Err(answered_error(operation, status, &body))
    }
}

/// The error an answer that is not a success carries:
/// `{"error": {"code": …, "message": …, "fields": {…}}}`.
fn answered_error(operation: &str, status: u16, body: &[u8]) -> BackendError {
    let error = serde_json::from_slice::<Value>(body)
        .ok()
        .and_then(|body| body.get("error").cloned());
    let Some(code) = error
        .as_ref()
        .and_then(|error| error.get("code"))
        .and_then(Value::as_str)
    else {
        return BackendError::Malformed(format!("{operation}: status {status} without a code"));
    };
    let message = error
        .as_ref()
        .and_then(|error| error.get("message"))
        .and_then(Value::as_str)
        .unwrap_or(code)
        .to_string();
    let fields = error
        .as_ref()
        .and_then(|error| error.get("fields"))
        .and_then(|fields| {
            serde_json::from_value::<BTreeMap<String, Vec<String>>>(fields.clone()).ok()
        })
        .map(FieldErrors);
    BackendError::Answered {
        status,
        code: code.to_string(),
        message,
        fields,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_answered_error_reads_the_code_message_and_fields() {
        let body = br#"{"error": {"code": "invalid", "message": "Check the name", "fields": {"name": ["too long"]}}}"#;
        let error = answered_error("files/update", 400, body);
        assert_eq!(error.code(), Some("invalid"));
        assert_eq!(error.to_string(), "Check the name");
        let BackendError::Answered { status, fields, .. } = error else {
            panic!("answered");
        };
        assert_eq!(status, 400);
        assert_eq!(fields, Some(FieldErrors::one("name", "too long")));
    }

    #[test]
    fn backend_answered_error_without_a_code_is_malformed() {
        let error = answered_error("files/update", 500, b"<html>oops</html>");
        assert!(matches!(error, BackendError::Malformed(_)));
        assert_eq!(error.code(), None);
    }
}
