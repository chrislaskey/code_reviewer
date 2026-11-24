//! The catalog API, `app_core::accounts::products` as JSON: what is for
//! sale, to anyone signed in, with whether they have it on a product's
//! page. Stripe's catalog is copied from the command line, see
//! `command_line::accounts_products`.

use axum::Json;
use axum::extract::{Path, State};
use uuid::Uuid;

use app_core::accounts::products::{self, ProductForSale, ProductShown};
use app_core::deployment::Deployment;

use crate::api_error::ApiError;
use crate::user_auth::CurrentUser;

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
