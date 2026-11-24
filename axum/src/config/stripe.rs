//! The server's Stripe client, chosen at start by `STRIPE_SECRET_KEY`,
//! `sk_test_...` or `sk_live_...`: unset or blank, the server has no
//! Stripe, sells nothing, and its webhook is a 404. `STRIPE_TAX` says who
//! sells: `managed_payments`, the default, Stripe as the merchant of
//! record, or `stripe_tax`, the business itself. See
//! `app_core::services::stripe` and `docs/design/purchasing.md`.

use std::env;
use std::sync::Arc;

use secrecy::SecretString;

use app_core::services::stripe::{Stripe, StripeTax};

pub fn open() -> Result<Option<Stripe>, String> {
    let tax = match env::var("STRIPE_TAX") {
        Ok(name) => StripeTax::from_name(&name).ok_or_else(|| {
            format!("STRIPE_TAX must be managed_payments or stripe_tax, got '{name}'")
        })?,
        Err(_) => StripeTax::default(),
    };
    let stripe = env::var("STRIPE_SECRET_KEY")
        .ok()
        .filter(|key| !key.trim().is_empty())
        .map(|key| Stripe::api(&SecretString::from(key), tax));
    match &stripe {
        Some(_) => tracing::info!("stripe: on, {tax:?}"),
        None => tracing::info!("stripe: off, no STRIPE_SECRET_KEY"),
    }
    Ok(stripe)
}

/// The webhook endpoint's signing secret, `whsec_...`, if the server has one.
#[derive(Clone, Default)]
pub struct StripeSigningSecret(pub Option<Arc<SecretString>>);

/// `STRIPE_WEBHOOK_SECRET`, unset or blank for none.
pub fn open_signing_secret() -> StripeSigningSecret {
    let secret = env::var("STRIPE_WEBHOOK_SECRET")
        .ok()
        .filter(|secret| !secret.trim().is_empty())
        .map(|secret| Arc::new(SecretString::from(secret)));
    tracing::info!(
        "stripe webhook: {}",
        if secret.is_some() {
            "on"
        } else {
            "off, no STRIPE_WEBHOOK_SECRET"
        }
    );
    StripeSigningSecret(secret)
}
