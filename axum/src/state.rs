//! What every handler may take from the server, `AppState`, each part
//! its own extractor through `FromRef`: the database with its buckets,
//! where its email goes, the rate limits, and the settings read at start,
//! see `config`.

use axum::extract::FromRef;

use app_core::deployment::Deployment;
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
