//! Plans paid every month or year: Stripe's subscriptions, one row of
//! `accounts_subscriptions` each, copied from Stripe when a plan's
//! checkout is paid and on every `customer.subscription.*` event, so a
//! change made anywhere, on Stripe's Link or by Stripe retrying a card,
//! lands here. Server only. Only an active subscription, or one in its
//! free days, gives its plan, see `plans`. The person switches, cancels
//! and resumes from its page, `update`, and changes their card on
//! Stripe's Customer Portal, `portal_create`.

use crate::data::accounts_prices as accounts_prices_records;
use crate::data::accounts_subscriptions as accounts_subscriptions_records;
use crate::data::accounts_users as accounts_users_records;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::accounts::checkouts::BACK_TO_APP_PATH;
use crate::accounts::online::ReturnTo;
use crate::accounts::prices::{self, Price, PriceInterval};
use crate::accounts::products::{self, ProductError, ProductKind};
use crate::accounts::purchases::Store;
use crate::app::users::User;
use crate::database::Transaction;
use crate::deployment::{Deployment, Role, now};
use crate::services::stripe::{Stripe, StripeError, StripeSubscription};

/// A record accepted by the subscriptions workflow.
pub(crate) struct AcceptedSubscription<'a>(&'a Subscription);

impl AcceptedSubscription<'_> {
    /// Borrows the accepted record for persistence.
    pub(crate) fn row(&self) -> &Subscription {
        self.0
    }
}

pub const TABLE: &str = "accounts_subscriptions";

/// Stripe's statuses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionStatus {
    Active,
    /// In its free days.
    Trialing,
    /// A renewal's payment failed, and Stripe is retrying it.
    PastDue,
    /// Stripe stopped retrying.
    Unpaid,
    /// The first payment is not made yet.
    Incomplete,
    IncompleteExpired,
    Paused,
    Canceled,
}

impl SubscriptionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            SubscriptionStatus::Active => "active",
            SubscriptionStatus::Trialing => "trialing",
            SubscriptionStatus::PastDue => "past_due",
            SubscriptionStatus::Unpaid => "unpaid",
            SubscriptionStatus::Incomplete => "incomplete",
            SubscriptionStatus::IncompleteExpired => "incomplete_expired",
            SubscriptionStatus::Paused => "paused",
            SubscriptionStatus::Canceled => "canceled",
        }
    }

    pub fn from_name(name: &str) -> Option<SubscriptionStatus> {
        match name {
            "active" => Some(SubscriptionStatus::Active),
            "trialing" => Some(SubscriptionStatus::Trialing),
            "past_due" => Some(SubscriptionStatus::PastDue),
            "unpaid" => Some(SubscriptionStatus::Unpaid),
            "incomplete" => Some(SubscriptionStatus::Incomplete),
            "incomplete_expired" => Some(SubscriptionStatus::IncompleteExpired),
            "paused" => Some(SubscriptionStatus::Paused),
            "canceled" => Some(SubscriptionStatus::Canceled),
            _ => None,
        }
    }

    /// Whether it gives its plan: paid, or in its free days. A plan not
    /// paid is no plan.
    pub fn gives_plan(self) -> bool {
        matches!(
            self,
            SubscriptionStatus::Active | SubscriptionStatus::Trialing
        )
    }

    /// Whether a payment failed and the person can fix it with another card.
    pub fn is_payment_failed(self) -> bool {
        matches!(
            self,
            SubscriptionStatus::PastDue | SubscriptionStatus::Unpaid
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Subscription {
    pub id: Uuid,
    pub user_id: Uuid,
    pub product_id: Uuid,
    pub price_id: Uuid,
    pub store: Store,
    /// The store's id, `sub_...` for Stripe.
    pub store_id: String,
    pub status: SubscriptionStatus,
    /// When the period paid for ends: the next renewal, or the end.
    pub current_period_end: Option<DateTime<Utc>>,
    pub cancel_at_period_end: bool,
    pub ended_at: Option<DateTime<Utc>>,
    /// The price a change waiting for the end of the period moves to.
    pub scheduled_price_id: Option<Uuid>,
    /// When it does.
    pub scheduled_at: Option<DateTime<Utc>>,
    pub inserted_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A subscription as its pages show it, with its plan and price.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubscriptionShown {
    #[serde(flatten)]
    pub subscription: Subscription,
    pub product_name: String,
    pub price: Price,
    pub scheduled_price: Option<Price>,
    pub gives_plan: bool,
}

/// A change the person makes to their plan, see
/// `docs/design/purchasing.md`, P7 and P8.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum SubscriptionChange {
    /// To another plan's price: at once, prorated, for a dearer price of
    /// the same interval or a monthly one to yearly; at the end of the
    /// period paid for otherwise.
    Switch { price_id: Uuid },
    /// A switch waiting for the end of the period, undone.
    SwitchUndo,
    /// Ends when the period paid for does.
    Cancel,
    /// A cancel undone, before the period ends.
    Resume,
}

#[derive(Debug, Error)]
pub enum SubscriptionError {
    #[error("only the server has subscriptions")]
    ServerOnly,
    #[error("No subscription {id} of yours")]
    NotFound { id: Uuid },
    #[error("This plan has ended")]
    Ended,
    #[error("This plan is managed in the store it was bought in")]
    OtherStore,
    #[error("The last payment failed: update your card first")]
    PaymentFailed,
    #[error("This plan ends with this period: resume it first")]
    Canceling,
    #[error("That price is not a plan for sale")]
    PriceNotForSale,
    #[error("This is the plan you are on")]
    SamePrice,
    #[error(transparent)]
    Stripe(#[from] StripeError),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl From<ProductError> for SubscriptionError {
    fn from(error: ProductError) -> SubscriptionError {
        match error {
            ProductError::NotFound { .. } => SubscriptionError::PriceNotForSale,
            ProductError::ServerOnly => SubscriptionError::ServerOnly,
            ProductError::Stripe(error) => SubscriptionError::Stripe(error),
            ProductError::Database(error) => SubscriptionError::Database(error),
        }
    }
}

/// The user's subscriptions, the newest first, ended ones too.
pub async fn list(
    deployment: &Deployment,
    user: &User,
) -> Result<Vec<SubscriptionShown>, SubscriptionError> {
    if deployment.role != Role::Server {
        return Err(SubscriptionError::ServerOnly);
    }
    let mut tx = deployment.begin().await?;
    let rows = accounts_subscriptions_records::list(&mut tx, user.id).await?;
    let mut shown = Vec::with_capacity(rows.len());
    for (subscription, product_name) in rows {
        shown.push(shown_in(&mut tx, subscription, product_name).await?);
    }
    tx.commit().await?;
    Ok(shown)
}

/// One of the user's subscriptions.
pub async fn get(
    deployment: &Deployment,
    user: &User,
    id: Uuid,
) -> Result<SubscriptionShown, SubscriptionError> {
    if deployment.role != Role::Server {
        return Err(SubscriptionError::ServerOnly);
    }
    let mut tx = deployment.begin().await?;
    let row = accounts_subscriptions_records::get(&mut tx, user.id, id)
        .await?
        .ok_or(SubscriptionError::NotFound { id })?;
    let shown = shown_in(&mut tx, row.0, row.1).await?;
    tx.commit().await?;
    Ok(shown)
}

/// Changes one of the user's plans on Stripe, then copies it back, so
/// the page shows the change at once.
pub async fn update(
    deployment: &Deployment,
    stripe: &Stripe,
    user: &User,
    id: Uuid,
    change: SubscriptionChange,
) -> Result<SubscriptionShown, SubscriptionError> {
    let shown = get(deployment, user, id).await?;
    let subscription = &shown.subscription;
    if subscription.store != Store::Stripe {
        return Err(SubscriptionError::OtherStore);
    }
    if subscription.ended_at.is_some() || subscription.status == SubscriptionStatus::Canceled {
        return Err(SubscriptionError::Ended);
    }
    let on_stripe = stripe.subscription_get(&subscription.store_id).await?;
    match change {
        SubscriptionChange::Switch { price_id } => {
            if !subscription.status.gives_plan() {
                return Err(SubscriptionError::PaymentFailed);
            }
            if subscription.cancel_at_period_end {
                return Err(SubscriptionError::Canceling);
            }
            if price_id == shown.price.id {
                return Err(SubscriptionError::SamePrice);
            }
            let target = plan_price_for_sale(deployment, price_id).await?;
            match (switch_waits(&shown.price, &target), target.interval) {
                (true, Some(interval)) => {
                    stripe
                        .subscription_price_update_at_period_end(
                            &on_stripe,
                            &target.stripe_id,
                            interval.as_str(),
                        )
                        .await?
                }
                _ => {
                    stripe
                        .subscription_price_update(&on_stripe, &target.stripe_id)
                        .await?
                }
            }
        }
        SubscriptionChange::SwitchUndo => stripe.subscription_schedule_release(&on_stripe).await?,
        SubscriptionChange::Cancel => {
            stripe
                .subscription_cancel_at_period_end(&on_stripe, true)
                .await?
        }
        SubscriptionChange::Resume => {
            stripe
                .subscription_cancel_at_period_end(&on_stripe, false)
                .await?
        }
    }
    let changed = stripe.subscription_get(&subscription.store_id).await?;
    let mut tx = deployment.begin().await?;
    upsert_from_stripe_in(&mut tx, &changed, now()).await?;
    tx.commit().await?;
    get(deployment, user, id).await
}

/// Stripe's Customer Portal for the plan's card and invoices: its page,
/// which comes back to the plan's, `{server_url}/accounts/subscriptions/{id}`,
/// or for the app to `checkouts::BACK_TO_APP_PATH`.
pub async fn portal_create(
    deployment: &Deployment,
    stripe: &Stripe,
    user: &User,
    id: Uuid,
    server_url: &str,
    return_to: ReturnTo,
) -> Result<String, SubscriptionError> {
    let shown = get(deployment, user, id).await?;
    if shown.subscription.store != Store::Stripe {
        return Err(SubscriptionError::OtherStore);
    }
    let customer_id = stripe
        .subscription_get(&shown.subscription.store_id)
        .await?
        .customer_id;
    let return_url = match return_to {
        ReturnTo::Web => format!("{server_url}/accounts/subscriptions/{id}"),
        ReturnTo::App => format!("{server_url}{BACK_TO_APP_PATH}"),
    };
    Ok(stripe
        .billing_portal_session_create(&customer_id, &return_url)
        .await?)
}

/// Whether a switch from one price to the other waits for the end of
/// the period paid for: a cheaper price of the same interval, or yearly
/// to monthly, so a paid year is not cut short. Stripe starts a new
/// period when the interval changes, so monthly to yearly is at once.
fn switch_waits(from: &Price, to: &Price) -> bool {
    match (from.interval, to.interval) {
        (Some(PriceInterval::Year), Some(PriceInterval::Month)) => true,
        (Some(PriceInterval::Month), Some(PriceInterval::Year)) => false,
        _ => to.amount < from.amount,
    }
}

async fn plan_price_for_sale(
    deployment: &Deployment,
    price_id: Uuid,
) -> Result<Price, SubscriptionError> {
    products::list(deployment)
        .await?
        .into_iter()
        .filter(|product| product.kind == ProductKind::Plan)
        .flat_map(|product| product.prices)
        .find(|price| price.id == price_id)
        .ok_or(SubscriptionError::PriceNotForSale)
}

/// The user's subscriptions, the newest first, for `plans`.
pub(crate) async fn list_in(
    tx: &mut Transaction,
    user_id: Uuid,
) -> Result<Vec<(Subscription, String)>, sqlx::Error> {
    accounts_subscriptions_records::list(&mut *tx, user_id).await
}

/// Persists a subscription accepted by Stripe or store-transaction processing.
pub(crate) async fn upsert_in(
    tx: &mut Transaction,
    subscription: &Subscription,
) -> Result<(), sqlx::Error> {
    accounts_subscriptions_records::upsert(tx, AcceptedSubscription(subscription)).await
}

/// The subscription as Stripe has it now: made, or updated in place by
/// its Stripe id. One whose customer is no account here, or whose price
/// is not in the catalog, is left out.
pub(crate) async fn upsert_from_stripe_in(
    tx: &mut Transaction,
    stripe_subscription: &StripeSubscription,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    let user_id = accounts_users_records::billing::get_id_by_stripe_customer_id(
        &mut *tx,
        &stripe_subscription.customer_id,
    )
    .await?;
    let price =
        accounts_prices_records::get_by_stripe_id(&mut *tx, &stripe_subscription.price_id).await?;
    let scheduled_price = match &stripe_subscription.scheduled_price_id {
        Some(stripe_id) => accounts_prices_records::get_by_stripe_id(&mut *tx, stripe_id).await?,
        None => None,
    };
    let (Some(user_id), Some(price)) = (user_id, price) else {
        tracing::warn!(
            "stripe subscription {} is for no account or price here, left out",
            stripe_subscription.id
        );
        return Ok(());
    };
    let Some(status) = SubscriptionStatus::from_name(&stripe_subscription.status) else {
        tracing::warn!(
            "stripe subscription {} has a status unknown here, {}, left out",
            stripe_subscription.id,
            stripe_subscription.status
        );
        return Ok(());
    };
    let subscription = Subscription {
        id: Uuid::now_v7(),
        user_id,
        product_id: price.product_id,
        price_id: price.id,
        store: Store::Stripe,
        store_id: stripe_subscription.id.clone(),
        status,
        current_period_end: stripe_subscription
            .current_period_end
            .and_then(|seconds| DateTime::from_timestamp(seconds, 0)),
        cancel_at_period_end: stripe_subscription.cancel_at_period_end,
        ended_at: stripe_subscription
            .ended_at
            .and_then(|seconds| DateTime::from_timestamp(seconds, 0)),
        scheduled_price_id: scheduled_price.as_ref().map(|price| price.id),
        scheduled_at: scheduled_price.as_ref().and(
            stripe_subscription
                .scheduled_at
                .and_then(|seconds| DateTime::from_timestamp(seconds, 0)),
        ),
        inserted_at: now,
        updated_at: now,
    };
    accounts_subscriptions_records::upsert(&mut *tx, AcceptedSubscription(&subscription)).await
}

async fn shown_in(
    tx: &mut Transaction,
    subscription: Subscription,
    product_name: String,
) -> Result<SubscriptionShown, sqlx::Error> {
    let price = prices::get_in(tx, subscription.price_id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)?;
    let scheduled_price = match subscription.scheduled_price_id {
        Some(price_id) => prices::get_in(tx, price_id).await?,
        None => None,
    };
    let gives_plan = subscription.status.gives_plan();
    Ok(SubscriptionShown {
        subscription,
        product_name,
        price,
        scheduled_price,
        gives_plan,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deployment::now;

    fn price(interval: PriceInterval, amount: i64) -> Price {
        Price {
            id: Uuid::now_v7(),
            product_id: Uuid::now_v7(),
            stripe_id: "price_test".to_string(),
            lookup_key: None,
            amount,
            currency: "usd".to_string(),
            interval: Some(interval),
            active: true,
            inserted_at: now(),
            updated_at: now(),
        }
    }

    #[test]
    fn subscriptions_switch_waits_for_cheaper_and_for_yearly_to_monthly() {
        use PriceInterval::{Month, Year};
        assert!(!switch_waits(&price(Month, 900), &price(Month, 1900)));
        assert!(switch_waits(&price(Month, 1900), &price(Month, 900)));
        assert!(!switch_waits(&price(Month, 900), &price(Year, 9000)));
        assert!(switch_waits(&price(Year, 9000), &price(Month, 900)));
        assert!(switch_waits(&price(Year, 19000), &price(Year, 9000)));
    }
}
