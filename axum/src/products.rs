//! The catalog API, `app_core::accounts::products` as JSON: what is for
//! sale, to anyone signed in, with whether they have it on a product's
//! page, and `app_web accounts products sync`, which
//! copies Stripe's catalog.

use axum::Json;
use axum::extract::{Path, State};
use uuid::Uuid;

use app_core::accounts::products::{self, ProductForSale, ProductShown};
use app_core::deployment::Deployment;
use app_core::services::stripe::Stripe;

use crate::api_error::ApiError;
use crate::current_user::CurrentUser;

/// `GET /api/accounts/products`
pub async fn list(
    State(deployment): State<Deployment>,
    CurrentUser(_user): CurrentUser,
) -> Result<Json<Vec<ProductForSale>>, ApiError> {
    Ok(Json(products::list(&deployment).await?))
}

/// `GET /api/accounts/products/{id}`
pub async fn get(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<Json<ProductShown>, ApiError> {
    Ok(Json(products::get(&deployment, &user, id).await?))
}

/// `app_web accounts products sync`: Stripe's whole catalog, copied.
pub async fn command(
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
