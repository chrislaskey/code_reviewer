//! Sending email: the one `send` the server calls with a `Mail` that
//! `mails` composed. `Log` writes the email to the log in full, for local
//! dev, where the code is read from the terminal; `Memory` keeps the
//! emails in a list, for tests to read the code from; `Resend` sends it
//! through Resend's API, behind the `mail-resend` feature, for
//! production. The server picks one at start, see `axum/src/mailer.rs`.

use std::sync::{Arc, Mutex, PoisonError};

use thiserror::Error;

/// One email, text only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mail {
    pub to: String,
    pub subject: String,
    pub text: String,
}

#[derive(Debug, Error)]
pub enum MailError {
    #[error("could not reach the mail service: {0}")]
    Transport(String),
    #[error("the mail service refused the email: {status} {body}")]
    Refused { status: u16, body: String },
}

/// Where the server's email goes.
#[derive(Clone)]
pub enum Mailer {
    /// Written to the log, every line, at `info`.
    Log,
    /// Kept in a list, `sent`, for tests.
    Memory(Arc<Mutex<Vec<Mail>>>),
    /// Sent through Resend.
    #[cfg(feature = "mail-resend")]
    Resend(resend::Resend),
}

/// Which one, and how many `Memory` holds; never the key.
impl std::fmt::Debug for Mailer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Mailer::Log => f.write_str("Log"),
            Mailer::Memory(_) => write!(f, "Memory({} sent)", self.sent().len()),
            #[cfg(feature = "mail-resend")]
            Mailer::Resend(_) => f.write_str("Resend"),
        }
    }
}

impl Mailer {
    pub fn memory() -> Mailer {
        Mailer::Memory(Arc::new(Mutex::new(Vec::new())))
    }

    pub async fn send(&self, mail: Mail) -> Result<(), MailError> {
        match self {
            Mailer::Log => {
                tracing::info!(
                    "email to {}\nSubject: {}\n\n{}",
                    mail.to,
                    mail.subject,
                    mail.text
                );
                Ok(())
            }
            Mailer::Memory(sent) => {
                sent.lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .push(mail);
                Ok(())
            }
            #[cfg(feature = "mail-resend")]
            Mailer::Resend(resend) => resend
                .send(mail)
                .await
                .inspect_err(|error| tracing::error!("email not sent: {error}")),
        }
    }

    /// The emails sent so far, oldest first; only `Memory` keeps them.
    pub fn sent(&self) -> Vec<Mail> {
        match self {
            Mailer::Memory(sent) => sent.lock().unwrap_or_else(PoisonError::into_inner).clone(),
            _ => Vec::new(),
        }
    }

    /// The last email sent to the address, if `Memory` kept one.
    pub fn last_sent_to(&self, to: &str) -> Option<Mail> {
        self.sent().into_iter().rev().find(|mail| mail.to == to)
    }
}

#[cfg(feature = "mail-resend")]
pub use resend::Resend;

#[cfg(feature = "mail-resend")]
mod resend {
    use secrecy::{ExposeSecret, SecretString};
    use serde::Serialize;

    use super::{Mail, MailError};

    const SEND_URL: &str = "https://api.resend.com/emails";

    /// Resend's API: one POST per email with the key as a bearer token.
    #[derive(Clone)]
    pub struct Resend {
        client: reqwest::Client,
        api_key: SecretString,
        from: String,
    }

    #[derive(Serialize)]
    struct SendRequest<'a> {
        from: &'a str,
        to: [&'a str; 1],
        subject: &'a str,
        text: &'a str,
    }

    impl Resend {
        /// With the API key and the sender address, `Name <address>` or
        /// a plain address.
        pub fn new(api_key: SecretString, from: String) -> Result<Resend, MailError> {
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .map_err(|error| MailError::Transport(error.to_string()))?;
            Ok(Resend {
                client,
                api_key,
                from,
            })
        }

        pub async fn send(&self, mail: Mail) -> Result<(), MailError> {
            let request = SendRequest {
                from: &self.from,
                to: [&mail.to],
                subject: &mail.subject,
                text: &mail.text,
            };
            let response = self
                .client
                .post(SEND_URL)
                .bearer_auth(self.api_key.expose_secret())
                .json(&request)
                .send()
                .await
                .map_err(|error| MailError::Transport(error.to_string()))?;
            let status = response.status();
            if status.is_success() {
                return Ok(());
            }
            let body = response.text().await.unwrap_or_default();
            Err(MailError::Refused {
                status: status.as_u16(),
                body,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mailer_memory_keeps_what_was_sent_and_log_keeps_nothing() {
        let mailer = Mailer::memory();
        let mail = Mail {
            to: "alice@example.com".to_string(),
            subject: "Hello".to_string(),
            text: "Hi".to_string(),
        };
        mailer.send(mail.clone()).await.expect("sent");
        assert_eq!(mailer.sent(), vec![mail.clone()]);
        assert_eq!(mailer.last_sent_to("alice@example.com"), Some(mail));
        assert_eq!(mailer.last_sent_to("bob@example.com"), None);

        let log = Mailer::Log;
        log.send(Mail {
            to: "bob@example.com".to_string(),
            subject: "Hello".to_string(),
            text: "Hi".to_string(),
        })
        .await
        .expect("logged");
        assert!(log.sent().is_empty());
    }
}
