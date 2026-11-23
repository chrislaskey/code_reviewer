//! The web server: `app_core` over a JSON API under `/api`, the sync
//! endpoints the apps converge with under `/api/sync`, the public website's
//! pages, see `public_pages`, and the Svelte pages (`../svelte/dist`) at
//! every other path. Every request is from a user, see `user_auth`.
//! Listens on `HOST`, 127.0.0.1 when unset, and `PORT`, 4000 when unset,
//! and opens the database `DATABASE_URL` names: a SQLite file, under `data/`
//! beside this crate when unset, or a PostgreSQL database. The email it
//! sends goes where `MAILER` says, see `config::mailer`, pictures go to the bucket
//! `STORAGE_BUCKET` names and files' bytes to `STORAGE_PRIVATE_BUCKET`'s,
//! see `config::storage`, and its accounts routes are rate limited, see
//! `rate_limits`. It sells through Stripe with `STRIPE_SECRET_KEY` and
//! `STRIPE_TAX`, see `config::stripe`, whose webhook needs
//! `STRIPE_WEBHOOK_SECRET`, and whose return links start with
//! `SERVER_URL`. It tells the apps what it is, `SERVER_NAME` and
//! `SERVER_DISPLAY_NAME`, and which other servers it lists,
//! `LISTED_SERVERS`, see `servers`; every setting is read at start, see
//! `config`.
//!
//! It runs the recurring jobs as they come due, see `jobs`. With
//! arguments it runs a command on that database instead and exits:
//! `jobs run` runs the jobs due, `jobs run <name>` that job now,
//! `ops staff create <email>` makes that account staff,
//! `accounts users create <email>` makes an account with a password,
//! `password <email>` gives it a new one and `delete <email>` deletes it,
//! and
//! `accounts products sync` copies Stripe's catalog, see `command_line`.
//!
//! The routes mirror core: a handler sits at the path of the core module
//! it calls, `app::posts` for `app_core::app::posts`, see
//! `docs/design/code-organization-axum-tauri-svelte.md`.

mod accounts;
mod api_error;
mod app;
mod command_line;
mod config;
mod jobs;
mod ops;
mod public_pages;
mod rate_limits;
mod request_locale;
mod router;
mod servers;
mod state;
mod sync;
#[cfg(test)]
mod tests;
mod user_auth;

use std::env;
use std::net::SocketAddr;
use std::time::Duration;

use app_core::deployment::FileStore;
use app_core::servers::settings::SettingsDocument;

use crate::config::{ServerUrl, TrialUsersMax};
use crate::rate_limits::RateLimits;
use crate::state::AppState;

#[tokio::main]
async fn main() -> Result<(), String> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "app_web=info,app_core=info,tower_http=info".into()),
        )
        .init();

    let deployment = config::database::open().await?;
    let mailer = config::mailer::open()?;
    let deployment = match config::storage::open_public()? {
        Some(public) => deployment.with_avatar_bucket(public),
        None => deployment,
    };
    let deployment = match config::storage::open_private()? {
        Some(private) => deployment.with_file_store(FileStore::Bucket(private)),
        None => deployment,
    };
    let stripe = config::stripe::open()?;
    let deployment = match &stripe {
        Some(stripe) => deployment.with_stripe(stripe.clone()),
        None => deployment,
    };
    let deployment = match config::app_store::open()? {
        Some(app_store) => deployment.with_app_store(app_store),
        None => deployment,
    };
    let arguments: Vec<String> = env::args().skip(1).collect();
    if !arguments.is_empty() {
        return command_line::run(&deployment, stripe.as_ref(), &arguments).await;
    }

    let rate_limits = RateLimits::new();
    let forgetting = rate_limits.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(60 * 60)).await;
            forgetting.forget_old();
        }
    });
    jobs::spawn(deployment.clone());
    let server_settings = config::server_settings()?;
    let state = AppState {
        deployment,
        mailer,
        rate_limits,
        trust_proxy_headers: config::trust_proxy_headers(),
        trial_users_max: TrialUsersMax::open()?,
        stripe_signing_secret: config::stripe::open_signing_secret(),
        stripe,
        server_url: ServerUrl::open()?,
        server_description: config::server_description(&server_settings)?,
        server_settings: std::sync::Arc::new(SettingsDocument::of(server_settings)),
        listed_servers: std::sync::Arc::new(config::listed_servers()?),
    };

    let address = SocketAddr::new(config::host()?, config::port()?);
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .map_err(|error| format!("could not listen on {address}: {error}"))?;
    tracing::info!("listening on http://{address}");

    let app = router::router(&config::svelte_dist(), &config::svelte_ops_dist(), state);
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .map_err(|error| format!("server stopped: {error}"))
}
