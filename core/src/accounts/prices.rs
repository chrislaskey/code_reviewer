//! A product's prices, one row of `accounts_prices` each, copied from
//! Stripe's catalog with the product, see `products`. Server only. A
//! price the catalog cannot sell, with no fixed amount, or billed every
//! few months, is kept inactive. The records are `data::accounts_prices`.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::data::accounts_prices::{self, Price, PriceInterval};

use crate::database::Transaction;
use crate::services::stripe::StripePrice;

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
    accounts_prices::upsert(&mut *tx, &price).await
}
