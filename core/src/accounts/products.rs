//! The catalog: Stripe's products and their prices, `prices`, copied to
//! `accounts_products` by the webhook's `product.*` and `price.*` events
//! and by `sync`. Server only. The code knows a price by its Stripe
//! lookup key, `Grant`, and sells only what it knows; what a product is
//! for and whether it can be bought twice are the grant's. See
//! `docs/design/purchasing.md`.

use crate::data::accounts_products::{self, Product};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::accounts::prices;
use crate::accounts::purchases;
use crate::data::accounts_prices::Price;
use crate::data::app_users::User;
use crate::database::Transaction;
use crate::deployment::{Deployment, Role, now};
use crate::services::stripe::{Stripe, StripeError, StripeProduct};

/// What a price gives the user who pays it, by its lookup key. No
/// feature needs one yet: these are placeholders, for a fork to rename.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Grant {
    /// `single_user_monthly`, `single_user_yearly`
    SingleUserSubscription,
    /// `license`
    License,
    /// `credits`
    Credits,
}

/// A plan is paid every month or year; a one-time product once.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductKind {
    Plan,
    OneTime,
}

impl Grant {
    pub fn from_lookup_key(lookup_key: &str) -> Option<Grant> {
        match lookup_key {
            "single_user_monthly" | "single_user_yearly" => Some(Grant::SingleUserSubscription),
            "license" => Some(Grant::License),
            "credits" => Some(Grant::Credits),
            _ => None,
        }
    }

    pub fn kind(self) -> ProductKind {
        match self {
            Grant::SingleUserSubscription => ProductKind::Plan,
            Grant::License | Grant::Credits => ProductKind::OneTime,
        }
    }

    /// Whether a one-time product can be bought again by someone who
    /// has it. Once, unless the grant says otherwise.
    pub fn is_repeatable(self) -> bool {
        matches!(self, Grant::Credits)
    }

    /// Days a plan is free before its first payment, Stripe's trial.
    pub fn free_days(self) -> Option<u32> {
        None
    }
}

/// A product the catalog sells, with the prices it sells it at: plans
/// by the month and the year, a one-time product its one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductForSale {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub grant: Grant,
    pub kind: ProductKind,
    pub is_repeatable: bool,
    pub free_days: Option<u32>,
    pub prices: Vec<Price>,
}

/// A product for sale as the user sees it: whether they have it, bought
/// and not refunded.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductShown {
    #[serde(flatten)]
    pub product: ProductForSale,
    pub owned: bool,
}

/// How many of each `sync` copied, active or not.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductsSynced {
    pub products: usize,
    pub prices: usize,
}

#[derive(Debug, Error)]
pub enum ProductError {
    #[error("only the server has the catalog")]
    ServerOnly,
    #[error("No product {id} is for sale")]
    NotFound { id: Uuid },
    #[error(transparent)]
    Stripe(#[from] StripeError),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// Every product for sale, plans first, then by name.
pub async fn list(deployment: &Deployment) -> Result<Vec<ProductForSale>, ProductError> {
    if deployment.role != Role::Server {
        return Err(ProductError::ServerOnly);
    }
    let mut tx = deployment.begin().await?;
    let products = accounts_products::list(&mut tx).await?;
    let prices = prices::list_in(&mut tx).await?;
    tx.commit().await?;
    let mut for_sale: Vec<ProductForSale> = products
        .into_iter()
        .filter_map(|product| for_sale(product, &prices))
        .collect();
    for_sale.sort_by(|a, b| (a.kind, &a.name).cmp(&(b.kind, &b.name)));
    Ok(for_sale)
}

/// The product for sale with the id, and whether the user has it.
pub async fn get(
    deployment: &Deployment,
    user: &User,
    id: Uuid,
) -> Result<ProductShown, ProductError> {
    let product = get_for_sale(deployment, id).await?;
    let mut tx = deployment.begin().await?;
    let owned = purchases::owns_in(&mut tx, user.id, id).await?;
    tx.commit().await?;
    Ok(ProductShown { product, owned })
}

/// The product for sale with the id.
pub(crate) async fn get_for_sale(
    deployment: &Deployment,
    id: Uuid,
) -> Result<ProductForSale, ProductError> {
    if deployment.role != Role::Server {
        return Err(ProductError::ServerOnly);
    }
    let mut tx = deployment.begin().await?;
    let product = accounts_products::get(&mut tx, id).await?;
    let prices = prices::list_in(&mut tx).await?;
    tx.commit().await?;
    product
        .and_then(|product| for_sale(product, &prices))
        .ok_or(ProductError::NotFound { id })
}

/// Copies Stripe's whole catalog: what Stripe no longer has, or has
/// archived, is no longer for sale. For the first time, as the webhook
/// only hears of changes, and for a webhook missed.
pub async fn sync(
    deployment: &Deployment,
    stripe: &Stripe,
) -> Result<ProductsSynced, ProductError> {
    if deployment.role != Role::Server {
        return Err(ProductError::ServerOnly);
    }
    let stripe_products = stripe.products_list().await?;
    let stripe_prices = stripe.prices_list().await?;
    let now = now();
    let mut tx = deployment.begin().await?;
    accounts_products::maintenance::deactivate_all(&mut tx, now).await?;
    prices::deactivate_all_in(&mut tx, now).await?;
    for stripe_product in &stripe_products {
        upsert_in(&mut tx, stripe_product, now).await?;
    }
    let mut prices_copied = 0;
    for stripe_price in &stripe_prices {
        let product =
            accounts_products::get_by_stripe_id(&mut tx, &stripe_price.product_id).await?;
        if let Some(product) = product {
            prices::upsert_in(&mut tx, product.id, stripe_price, now).await?;
            prices_copied += 1;
        }
    }
    tx.commit().await?;
    Ok(ProductsSynced {
        products: stripe_products.len(),
        prices: prices_copied,
    })
}

/// The product as Stripe has it now: made, or updated in place by its
/// Stripe id. Answers its id.
pub(crate) async fn upsert_in(
    tx: &mut Transaction,
    stripe_product: &StripeProduct,
    now: DateTime<Utc>,
) -> Result<Uuid, sqlx::Error> {
    let product = Product {
        id: Uuid::now_v7(),
        stripe_id: stripe_product.id.clone(),
        name: stripe_product.name.clone(),
        description: stripe_product.description.clone(),
        active: stripe_product.active,
        inserted_at: now,
        updated_at: now,
    };
    accounts_products::upsert(&mut *tx, &product).await?;
    let upserted = accounts_products::get_by_stripe_id(&mut *tx, &stripe_product.id).await?;
    upserted
        .map(|product| product.id)
        .ok_or(sqlx::Error::RowNotFound)
}

/// A product Stripe deleted, no longer for sale.
pub(crate) async fn deactivate_by_stripe_id_in(
    tx: &mut Transaction,
    stripe_id: &str,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    accounts_products::deactivate_by_stripe_id(&mut *tx, stripe_id, now).await
}

/// The product with the prices of it the catalog sells, if any: active,
/// with a lookup key the code knows, all of the first one's grant, and
/// paid every month or year for a plan, once for a one-time product.
fn for_sale(product: Product, prices: &[Price]) -> Option<ProductForSale> {
    if !product.active {
        return None;
    }
    let mut sellable: Vec<(Grant, &Price)> = prices
        .iter()
        .filter(|price| price.product_id == product.id && price.active)
        .filter_map(|price| {
            let grant = Grant::from_lookup_key(price.lookup_key.as_deref()?)?;
            let fits = match grant.kind() {
                ProductKind::Plan => price.interval.is_some(),
                ProductKind::OneTime => price.interval.is_none(),
            };
            fits.then_some((grant, price))
        })
        .collect();
    sellable.sort_by_key(|(_, price)| (price.interval, price.amount));
    let grant = sellable.first()?.0;
    let prices = sellable
        .into_iter()
        .filter(|(price_grant, _)| *price_grant == grant)
        .map(|(_, price)| price.clone())
        .collect();
    Some(ProductForSale {
        id: product.id,
        name: product.name,
        description: product.description,
        grant,
        kind: grant.kind(),
        is_repeatable: grant.is_repeatable(),
        free_days: grant.free_days(),
        prices,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::accounts_prices::PriceInterval;
    use crate::database::with_each_database;
    use crate::services::stripe::StripePrice;

    fn stripe_product(id: &str, name: &str) -> StripeProduct {
        StripeProduct {
            id: id.to_string(),
            name: name.to_string(),
            description: None,
            active: true,
        }
    }

    fn stripe_price(
        id: &str,
        product_id: &str,
        lookup_key: Option<&str>,
        amount: i64,
        interval: Option<&str>,
    ) -> StripePrice {
        StripePrice {
            id: id.to_string(),
            product_id: product_id.to_string(),
            lookup_key: lookup_key.map(str::to_string),
            amount: Some(amount),
            currency: "usd".to_string(),
            interval: interval.map(str::to_string),
            active: true,
        }
    }

    /// The placeholders, and what the catalog leaves out: a lookup key
    /// the code does not know, and a plan's key on a one-time price.
    fn stripe_catalog() -> Stripe {
        let stripe = Stripe::memory();
        stripe.memory_product_put(stripe_product("prod_single", "Single user subscription"));
        stripe.memory_product_put(stripe_product("prod_license", "License"));
        stripe.memory_product_put(stripe_product("prod_credits", "Credits"));
        stripe.memory_product_put(stripe_product("prod_unknown", "Unknown"));
        stripe.memory_product_put(stripe_product("prod_mismatched", "Mismatched"));
        for price in [
            stripe_price(
                "price_yearly",
                "prod_single",
                Some("single_user_yearly"),
                9000,
                Some("year"),
            ),
            stripe_price(
                "price_monthly",
                "prod_single",
                Some("single_user_monthly"),
                900,
                Some("month"),
            ),
            stripe_price("price_license", "prod_license", Some("license"), 4900, None),
            stripe_price("price_credits", "prod_credits", Some("credits"), 500, None),
            stripe_price(
                "price_unknown",
                "prod_unknown",
                Some("not_a_grant"),
                100,
                None,
            ),
            stripe_price(
                "price_mismatched",
                "prod_mismatched",
                Some("single_user_monthly"),
                100,
                None,
            ),
        ] {
            stripe.memory_price_put(price);
        }
        stripe
    }

    #[tokio::test]
    async fn products_sync_then_list_sells_what_the_code_knows() {
        with_each_database(|database| async move {
            let deployment = Deployment::new(database, Role::Server);
            let synced = sync(&deployment, &stripe_catalog()).await.expect("synced");
            assert_eq!(
                synced,
                ProductsSynced {
                    products: 5,
                    prices: 6
                }
            );

            let for_sale = list(&deployment).await.expect("the catalog");
            let names: Vec<&str> = for_sale
                .iter()
                .map(|product| product.name.as_str())
                .collect();
            assert_eq!(names, ["Single user subscription", "Credits", "License"]);
            let plan = &for_sale[0];
            assert_eq!(plan.grant, Grant::SingleUserSubscription);
            assert_eq!(plan.kind, ProductKind::Plan);
            let intervals: Vec<_> = plan.prices.iter().map(|price| price.interval).collect();
            assert_eq!(
                intervals,
                [Some(PriceInterval::Month), Some(PriceInterval::Year)]
            );
            assert!(for_sale[1].is_repeatable);
            assert!(!for_sale[2].is_repeatable);
            assert_eq!(for_sale[2].free_days, None);

            let license = get_for_sale(&deployment, for_sale[2].id)
                .await
                .expect("the license");
            assert_eq!(license, for_sale[2]);
            let error = get_for_sale(&deployment, Uuid::now_v7())
                .await
                .expect_err("no product");
            assert!(matches!(error, ProductError::NotFound { .. }));
        })
        .await;
    }

    #[tokio::test]
    async fn products_sync_stops_selling_what_stripe_archived_or_dropped() {
        with_each_database(|database| async move {
            let deployment = Deployment::new(database, Role::Server);
            let stripe = stripe_catalog();
            sync(&deployment, &stripe).await.expect("synced");
            let license_id = list(&deployment).await.expect("the catalog")[2].id;

            let mut archived = stripe_product("prod_credits", "Credits");
            archived.active = false;
            stripe.memory_product_put(archived);
            if let Stripe::Memory(memory) = &stripe {
                let mut memory = memory.lock().expect("the memory");
                memory.products.remove("prod_single");
                memory
                    .prices
                    .retain(|_, price| price.product_id != "prod_single");
            }
            sync(&deployment, &stripe).await.expect("synced again");

            let for_sale = list(&deployment).await.expect("the catalog");
            let names: Vec<&str> = for_sale
                .iter()
                .map(|product| product.name.as_str())
                .collect();
            assert_eq!(names, ["License"]);
            assert_eq!(for_sale[0].id, license_id, "a product keeps its id");
        })
        .await;
    }

    #[test]
    fn grants_are_placeholders_once_unless_repeatable() {
        assert_eq!(
            Grant::from_lookup_key("single_user_yearly"),
            Some(Grant::SingleUserSubscription)
        );
        assert_eq!(Grant::from_lookup_key("license"), Some(Grant::License));
        assert_eq!(Grant::from_lookup_key("License"), None);
        assert!(Grant::Credits.is_repeatable());
        assert!(!Grant::License.is_repeatable());
        assert_eq!(Grant::SingleUserSubscription.kind(), ProductKind::Plan);
    }
}
