//! A product's prices, one row of `accounts_prices` each, copied from
//! Stripe's catalog with the product, see `products`. Server only. A
//! price the catalog cannot sell, with no fixed amount, or billed every
//! few months, is kept inactive.

use crate::data::accounts_prices;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::database::Transaction;
use crate::services::stripe::StripePrice;

/// A record accepted by the prices workflow.
pub(crate) struct AcceptedPrice<'a>(&'a Price);

impl AcceptedPrice<'_> {
    /// Borrows the accepted record for persistence.
    pub(crate) fn row(&self) -> &Price {
        self.0
    }
}

pub const TABLE: &str = "accounts_prices";

/// How often a plan's price is paid; a one-time price has none.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PriceInterval {
    Month,
    Year,
}

impl PriceInterval {
    pub fn as_str(self) -> &'static str {
        match self {
            PriceInterval::Month => "month",
            PriceInterval::Year => "year",
        }
    }

    /// Stripe's word, or the column's; none for any other.
    pub fn from_name(name: &str) -> Option<PriceInterval> {
        match name {
            "month" => Some(PriceInterval::Month),
            "year" => Some(PriceInterval::Year),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Price {
    pub id: Uuid,
    pub product_id: Uuid,
    /// `price_...`
    pub stripe_id: String,
    /// What the code knows the price by, see `products::Grant`.
    pub lookup_key: Option<String>,
    /// In the currency's smallest unit, cents for `usd`.
    pub amount: i64,
    /// Lowercase, `usd`.
    pub currency: String,
    pub interval: Option<PriceInterval>,
    pub active: bool,
    pub inserted_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// The price as Stripe has it now, under the product with the id: made,
/// or updated in place by its Stripe id.
pub(crate) async fn upsert_in(
    tx: &mut Transaction,
    product_id: Uuid,
    stripe_price: &StripePrice,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    let interval = stripe_price.interval.as_deref();
    let sellable_interval = interval.is_none_or(|name| PriceInterval::from_name(name).is_some());
    let price = Price {
        id: Uuid::now_v7(),
        product_id,
        stripe_id: stripe_price.id.clone(),
        lookup_key: stripe_price.lookup_key.clone(),
        amount: stripe_price.amount.unwrap_or(0),
        currency: stripe_price.currency.clone(),
        interval: interval.and_then(PriceInterval::from_name),
        active: stripe_price.active && stripe_price.amount.is_some() && sellable_interval,
        inserted_at: now,
        updated_at: now,
    };
    accounts_prices::upsert(&mut *tx, AcceptedPrice(&price)).await
}

/// Every price, active or not.
pub(crate) async fn list_in(tx: &mut Transaction) -> Result<Vec<Price>, sqlx::Error> {
    accounts_prices::list(&mut *tx).await
}

pub(crate) async fn get_in(tx: &mut Transaction, id: Uuid) -> Result<Option<Price>, sqlx::Error> {
    accounts_prices::get(&mut *tx, id).await
}

/// Every price inactive, before a sync makes active again what Stripe
/// still has.
pub(crate) async fn deactivate_all_in(
    tx: &mut Transaction,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    accounts_prices::maintenance::deactivate_all(&mut *tx, now).await
}
