//! What every handler may take from the server, `AppState`, each part
//! its own extractor through `FromRef`: the database with its buckets,
//! where its email goes, the rate limits, and the settings read at start,
//! see `config`, among them what it says it is, what it offers and the
//! servers it lists.

use std::sync::Arc;

use axum::extract::FromRef;

use app_core::deployment::Deployment;
use app_core::servers::settings::SettingsDocument;
use app_core::servers::{ListedServers, ServerDescription};
use app_core::services::mailer::Mailer;
use app_core::services::stripe::Stripe;

use crate::config::stripe::StripeSigningSecret;
use crate::config::{ServerUrl, TrialUsersMax};
use crate::rate_limits::RateLimits;

/// The server's state; whether the client IP is the proxy's header, see
/// `rate_limits`.
#[derive(Clone)]
pub struct AppState {
    pub deployment: Deployment,
    pub mailer: Mailer,
    pub rate_limits: RateLimits,
    pub trust_proxy_headers: bool,
    pub trial_users_max: TrialUsersMax,
    pub stripe_signing_secret: StripeSigningSecret,
    pub stripe: Option<Stripe>,
    pub server_url: ServerUrl,
    pub server_description: ServerDescription,
    pub server_settings: Arc<SettingsDocument>,
    pub listed_servers: Arc<ListedServers>,
}

impl FromRef<AppState> for Arc<SettingsDocument> {
    fn from_ref(state: &AppState) -> Arc<SettingsDocument> {
        state.server_settings.clone()
    }
}

impl FromRef<AppState> for ServerDescription {
    fn from_ref(state: &AppState) -> ServerDescription {
        state.server_description.clone()
    }
}

impl FromRef<AppState> for Arc<ListedServers> {
    fn from_ref(state: &AppState) -> Arc<ListedServers> {
        state.listed_servers.clone()
    }
}

impl FromRef<AppState> for Deployment {
    fn from_ref(state: &AppState) -> Deployment {
        state.deployment.clone()
    }
}

impl FromRef<AppState> for Mailer {
    fn from_ref(state: &AppState) -> Mailer {
        state.mailer.clone()
    }
}

impl FromRef<AppState> for RateLimits {
    fn from_ref(state: &AppState) -> RateLimits {
        state.rate_limits.clone()
    }
}

impl FromRef<AppState> for TrialUsersMax {
    fn from_ref(state: &AppState) -> TrialUsersMax {
        state.trial_users_max
    }
}

impl FromRef<AppState> for StripeSigningSecret {
    fn from_ref(state: &AppState) -> StripeSigningSecret {
        state.stripe_signing_secret.clone()
    }
}

impl FromRef<AppState> for Option<Stripe> {
    fn from_ref(state: &AppState) -> Option<Stripe> {
        state.stripe.clone()
    }
}

impl FromRef<AppState> for ServerUrl {
    fn from_ref(state: &AppState) -> ServerUrl {
        state.server_url.clone()
    }
}
