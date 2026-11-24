//! `app_web accounts products sync`: Stripe's whole catalog, copied, see
//! `app_core::accounts::products`.

use app_core::accounts::products;
use app_core::deployment::Deployment;
use app_core::services::stripe::Stripe;

/// `app_web accounts products sync`: Stripe's whole catalog, copied.
pub async fn run(
    deployment: &Deployment,
    stripe: Option<&Stripe>,
    words: &[&str],
) -> Result<(), String> {
    match words {
        ["sync"] => {
            let stripe = stripe.ok_or("accounts products sync needs STRIPE_SECRET_KEY")?;
            let synced = products::sync(deployment, stripe)
                .await
                .map_err(|error| error.to_string())?;
            println!(
                "copied {} products and {} prices from Stripe",
                synced.products, synced.prices
            );
            Ok(())
        }
        _ => Err("accounts products commands: sync".to_string()),
    }
}
