//! Where the server's email goes, chosen at start by `MAILER`: unset or
//! `log`, the emails are written to the log in full, which is how a dev
//! reads a sign-in code; `resend`, they are sent through Resend with
//! `RESEND_API_KEY` and `MAIL_FROM`, the sender as `Name <address>` or a
//! plain address, see `app_core::services::mailer`.

use std::env;

use app_core::services::mailer::{Mailer, Resend};
use secrecy::SecretString;

pub fn open() -> Result<Mailer, String> {
    let choice = env::var("MAILER").unwrap_or_else(|_| "log".to_string());
    let api_key = env::var("RESEND_API_KEY").ok().map(SecretString::from);
    let from = env::var("MAIL_FROM").ok();
    let mailer = from_values(&choice, api_key, from)?;
    tracing::info!(
        "mailer: {}",
        match mailer {
            Mailer::Log => "the log",
            Mailer::Resend(_) => "Resend",
            Mailer::Memory(_) => "memory",
        }
    );
    Ok(mailer)
}

/// The mailer the values name, with what each needs.
fn from_values(
    choice: &str,
    api_key: Option<SecretString>,
    from: Option<String>,
) -> Result<Mailer, String> {
    match choice {
        "log" => Ok(Mailer::Log),
        "resend" => {
            let api_key = api_key.ok_or("MAILER=resend needs RESEND_API_KEY")?;
            let from = from
                .filter(|from| !from.trim().is_empty())
                .ok_or("MAILER=resend needs MAIL_FROM, the sender address")?;
            let resend = Resend::new(api_key, from).map_err(|error| error.to_string())?;
            Ok(Mailer::Resend(resend))
        }
        other => Err(format!("MAILER must be log or resend, got '{other}'")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mailer_from_values_needs_the_key_and_the_sender_for_resend() {
        assert!(matches!(from_values("log", None, None), Ok(Mailer::Log)));
        let error = from_values("resend", None, None).expect_err("no key");
        assert!(error.contains("RESEND_API_KEY"));
        let key = || Some(SecretString::from("re_test".to_string()));
        let error = from_values("resend", key(), Some("  ".to_string())).expect_err("no sender");
        assert!(error.contains("MAIL_FROM"));
        assert!(matches!(
            from_values(
                "resend",
                key(),
                Some("Starter Pack <hello@example.com>".to_string())
            ),
            Ok(Mailer::Resend(_))
        ));
        let error = from_values("smtp", None, None).expect_err("unknown");
        assert!(error.contains("log or resend"));
    }
}
