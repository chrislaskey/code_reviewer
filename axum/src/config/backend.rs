//! Which backend owns the server's data, chosen at start by `BACKEND`:
//! unset or `rust`, the Rust code in core, over the database
//! `DATABASE_URL` names, see `config::database`; `remote`, a server that
//! owns the database alone, such as `core_elixir/`, at `BACKEND_URL`,
//! called with the service token `BACKEND_SERVICE_TOKEN`, see
//! `docs/design/backend-seam.md`. A remote backend's server opens no
//! database, so it refuses to start with a `DATABASE_URL`.

use std::env;

use app_core::deployment::Deployment;
use app_core::services::backend::RemoteBackend;
use secrecy::SecretString;

pub async fn open() -> Result<Deployment, String> {
    let choice = env::var("BACKEND").unwrap_or_else(|_| "rust".to_string());
    match choice.as_str() {
        "rust" => super::database::open().await,
        "remote" => {
            let backend = remote_from_values(
                env::var("BACKEND_URL").ok(),
                env::var("BACKEND_SERVICE_TOKEN")
                    .ok()
                    .map(SecretString::from),
                env::var("DATABASE_URL").is_ok(),
            )?;
            Ok(Deployment::new_with_remote_backend(backend))
        }
        other => Err(format!("BACKEND must be rust or remote, got '{other}'")),
    }
}

/// The remote backend the values name, with what it needs.
fn remote_from_values(
    url: Option<String>,
    service_token: Option<SecretString>,
    has_database_url: bool,
) -> Result<RemoteBackend, String> {
    if has_database_url {
        return Err(
            "BACKEND=remote opens no database: the backend owns it, so unset DATABASE_URL"
                .to_string(),
        );
    }
    let url = url
        .filter(|url| url.starts_with("http://") || url.starts_with("https://"))
        .ok_or("BACKEND=remote needs BACKEND_URL, http:// or https://")?;
    let service_token = service_token.ok_or("BACKEND=remote needs BACKEND_SERVICE_TOKEN")?;
    tracing::info!("backend: remote, at {url}");
    RemoteBackend::new(&url, service_token).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token() -> Option<SecretString> {
        Some(SecretString::from("a-service-token"))
    }

    #[test]
    fn backend_remote_needs_its_url_and_token() {
        assert!(remote_from_values(Some("http://localhost:4100".into()), token(), false).is_ok());
        assert!(remote_from_values(None, token(), false).is_err());
        assert!(remote_from_values(Some("localhost:4100".into()), token(), false).is_err());
        assert!(remote_from_values(Some("http://localhost:4100".into()), None, false).is_err());
    }

    #[test]
    fn backend_remote_refuses_a_database_url() {
        let error = remote_from_values(Some("http://localhost:4100".into()), token(), true)
            .expect_err("refused");
        assert!(error.contains("DATABASE_URL"));
    }
}
