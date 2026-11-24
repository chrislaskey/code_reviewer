//! Calls to Stripe's API, answering plain data: `Api` calls Stripe with
//! the server's secret key, `STRIPE_SECRET_KEY`; `Memory` keeps the
//! objects a test put in it, so tests never reach the network. The
//! server picks one at start, see `axum/src/stripe.rs`. Only what the
//! purchasing design needs is here, see `docs/design/purchasing.md`.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

use futures_util::TryStreamExt;
use secrecy::{ExposeSecret, SecretString};
use stripe::{IdempotencyKey, RequestStrategy};
use stripe_client_core::StripeRequest;
use thiserror::Error;

/// A product in Stripe's catalog.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StripeProduct {
    /// `prod_...`
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub active: bool,
}

/// A price of a product in Stripe's catalog.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StripePrice {
    /// `price_...`
    pub id: String,
    pub product_id: String,
    pub lookup_key: Option<String>,
    /// In the currency's smallest unit; none for a price Stripe works
    /// out some other way, which is never for sale here.
    pub amount: Option<i64>,
    /// Lowercase, `usd`.
    pub currency: String,
    /// None for a one-time price, else Stripe's `day`, `week`, `month`
    /// or `year`, every one interval long.
    pub interval: Option<String>,
    pub active: bool,
}

/// Who sells, and so who works out the tax, `STRIPE_TAX`: Stripe as the
/// merchant of record, the default, or the server's business with
/// Stripe Tax. See `docs/design/purchasing.md`, Tax.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StripeTax {
    #[default]
    ManagedPayments,
    StripeTax,
}

impl StripeTax {
    /// `managed_payments` or `stripe_tax`.
    pub fn from_name(name: &str) -> Option<StripeTax> {
        match name {
            "managed_payments" => Some(StripeTax::ManagedPayments),
            "stripe_tax" => Some(StripeTax::StripeTax),
            _ => None,
        }
    }
}

/// A checkout for a one-time price, or a plan's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckoutMode {
    Payment,
    Subscription,
}

/// What a new Checkout Session is for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StripeCheckoutSessionNew {
    /// `cus_...`
    pub customer_id: String,
    /// `price_...`
    pub price_id: String,
    pub mode: CheckoutMode,
    /// Our checkout's id, which the session carries back.
    pub client_reference_id: String,
    pub success_url: String,
    pub cancel_url: String,
    pub free_days: Option<u32>,
    pub tax: StripeTax,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StripeCheckoutSessionStatus {
    Open,
    Complete,
    Expired,
}

/// Where a session's payment is: paid, or not yet; a payment that
/// settles later, a bank debit, is `Processing` until it does, then
/// paid, or `Failed`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StripePaymentStatus {
    Paid,
    Unpaid,
    Processing,
    Failed,
}

/// A Checkout Session as Stripe has it now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StripeCheckoutSession {
    /// `cs_...`
    pub id: String,
    /// Stripe's page, while the session is open.
    pub url: Option<String>,
    pub client_reference_id: Option<String>,
    pub status: StripeCheckoutSessionStatus,
    pub payment_status: StripePaymentStatus,
    /// `pi_...`, for a one-time price.
    pub payment_intent_id: Option<String>,
    /// `sub_...`, for a plan.
    pub subscription_id: Option<String>,
    pub amount_total: Option<i64>,
    pub currency: Option<String>,
}

/// A subscription as Stripe has it now, for its one price.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StripeSubscription {
    /// `sub_...`
    pub id: String,
    /// `cus_...`
    pub customer_id: String,
    /// `si_...`, the item that has the price.
    pub item_id: String,
    /// `price_...`
    pub price_id: String,
    /// Stripe's word, `active`, `past_due`, ...
    pub status: String,
    /// Unix seconds.
    pub current_period_end: Option<i64>,
    /// Ends when the period paid for does, unless resumed.
    pub cancel_at_period_end: bool,
    /// Unix seconds.
    pub ended_at: Option<i64>,
    /// `sub_sched_...`, while a change waits for the end of the period.
    pub schedule_id: Option<String>,
    /// The price the waiting change moves to, `price_...`.
    pub scheduled_price_id: Option<String>,
    /// When it does, Unix seconds.
    pub scheduled_at: Option<i64>,
}

/// A charge, as far as a refund goes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StripeCharge {
    /// `ch_...`
    pub id: String,
    pub payment_intent_id: Option<String>,
    /// Refunded in full.
    pub refunded: bool,
}

#[derive(Debug, Error)]
pub enum StripeError {
    #[error("Stripe has no {object} {id}")]
    NotFound { object: &'static str, id: String },
    #[error("Stripe refused the request: {0}")]
    Refused(String),
    #[error("could not reach Stripe: {0}")]
    Transport(String),
}

/// Where the server's calls to Stripe go.
#[derive(Clone)]
pub enum Stripe {
    Api(Arc<StripeApi>),
    Memory(Arc<Mutex<StripeMemory>>),
}

/// Stripe's API, and who sells.
pub struct StripeApi {
    client: stripe::Client,
    tax: StripeTax,
}

/// Which one; never the key.
impl std::fmt::Debug for Stripe {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Stripe::Api(_) => f.write_str("Api"),
            Stripe::Memory(_) => f.write_str("Memory"),
        }
    }
}

/// What `Memory` holds, by Stripe's ids, and the sessions asked for.
#[derive(Debug, Default)]
pub struct StripeMemory {
    pub tax: StripeTax,
    pub products: BTreeMap<String, StripeProduct>,
    pub prices: BTreeMap<String, StripePrice>,
    /// Customer ids by email.
    pub customers: BTreeMap<String, String>,
    pub checkout_sessions: BTreeMap<String, StripeCheckoutSession>,
    pub checkout_sessions_asked: Vec<StripeCheckoutSessionNew>,
    pub charges: BTreeMap<String, StripeCharge>,
    pub subscriptions: BTreeMap<String, StripeSubscription>,
}

impl Stripe {
    /// Stripe's API with the secret key, `sk_...`, selling as `tax` says.
    pub fn api(secret_key: &SecretString, tax: StripeTax) -> Stripe {
        let client = stripe::Client::new(secret_key.expose_secret());
        Stripe::Api(Arc::new(StripeApi { client, tax }))
    }

    pub fn tax(&self) -> StripeTax {
        match self {
            Stripe::Api(api) => api.tax,
            Stripe::Memory(memory) => lock(memory).tax,
        }
    }

    pub fn memory() -> Stripe {
        Stripe::Memory(Arc::new(Mutex::new(StripeMemory::default())))
    }

    /// Puts a product in `Memory`, or replaces the one with its id.
    pub fn memory_product_put(&self, product: StripeProduct) {
        if let Stripe::Memory(memory) = self {
            lock(memory).products.insert(product.id.clone(), product);
        }
    }

    /// Puts a price in `Memory`, or replaces the one with its id.
    pub fn memory_price_put(&self, price: StripePrice) {
        if let Stripe::Memory(memory) = self {
            lock(memory).prices.insert(price.id.clone(), price);
        }
    }

    /// Every product, active or archived.
    pub async fn products_list(&self) -> Result<Vec<StripeProduct>, StripeError> {
        match self {
            Stripe::Api(api) => {
                let products: Vec<stripe_shared::Product> =
                    stripe_product::product::ListProduct::new()
                        .limit(100)
                        .paginate()
                        .stream(&api.client)
                        .try_collect()
                        .await
                        .map_err(StripeError::from_api)?;
                Ok(products.iter().map(product_from).collect())
            }
            Stripe::Memory(memory) => Ok(lock(memory).products.values().cloned().collect()),
        }
    }

    /// Every price, active or archived.
    pub async fn prices_list(&self) -> Result<Vec<StripePrice>, StripeError> {
        match self {
            Stripe::Api(api) => {
                let prices: Vec<stripe_shared::Price> = stripe_product::price::ListPrice::new()
                    .limit(100)
                    .paginate()
                    .stream(&api.client)
                    .try_collect()
                    .await
                    .map_err(StripeError::from_api)?;
                Ok(prices.iter().map(price_from).collect())
            }
            Stripe::Memory(memory) => Ok(lock(memory).prices.values().cloned().collect()),
        }
    }

    pub async fn product_get(&self, id: &str) -> Result<StripeProduct, StripeError> {
        match self {
            Stripe::Api(api) => {
                let product = stripe_product::product::RetrieveProduct::new(id)
                    .send(&api.client)
                    .await
                    .map_err(|error| StripeError::from_api_for("product", id, error))?;
                Ok(product_from(&product))
            }
            Stripe::Memory(memory) => {
                lock(memory)
                    .products
                    .get(id)
                    .cloned()
                    .ok_or_else(|| StripeError::NotFound {
                        object: "product",
                        id: id.to_string(),
                    })
            }
        }
    }

    pub async fn price_get(&self, id: &str) -> Result<StripePrice, StripeError> {
        match self {
            Stripe::Api(api) => {
                let price = stripe_product::price::RetrievePrice::new(id)
                    .send(&api.client)
                    .await
                    .map_err(|error| StripeError::from_api_for("price", id, error))?;
                Ok(price_from(&price))
            }
            Stripe::Memory(memory) => {
                lock(memory)
                    .prices
                    .get(id)
                    .cloned()
                    .ok_or_else(|| StripeError::NotFound {
                        object: "price",
                        id: id.to_string(),
                    })
            }
        }
    }

    /// Puts a Checkout Session in `Memory`, as a payment would leave it.
    pub fn memory_checkout_session_put(&self, session: StripeCheckoutSession) {
        if let Stripe::Memory(memory) = self {
            lock(memory)
                .checkout_sessions
                .insert(session.id.clone(), session);
        }
    }

    pub fn memory_subscription_put(&self, subscription: StripeSubscription) {
        if let Stripe::Memory(memory) = self {
            lock(memory)
                .subscriptions
                .insert(subscription.id.clone(), subscription);
        }
    }

    pub async fn subscription_get(&self, id: &str) -> Result<StripeSubscription, StripeError> {
        match self {
            Stripe::Api(api) => {
                let subscription = stripe_billing::subscription::RetrieveSubscription::new(id)
                    .send(&api.client)
                    .await
                    .map_err(|error| StripeError::from_api_for("subscription", id, error))?;
                let schedule = match &subscription.schedule {
                    Some(schedule) => Some(api.schedule_get(schedule.id().as_str()).await?),
                    None => None,
                };
                subscription_from(&subscription, schedule.as_ref())
            }
            Stripe::Memory(memory) => {
                lock(memory)
                    .subscriptions
                    .get(id)
                    .cloned()
                    .ok_or_else(|| StripeError::NotFound {
                        object: "subscription",
                        id: id.to_string(),
                    })
            }
        }
    }

    /// The subscription's price changed now, prorated: a dearer one is
    /// paid for the rest of the period at once.
    pub async fn subscription_price_update(
        &self,
        subscription: &StripeSubscription,
        price_id: &str,
    ) -> Result<(), StripeError> {
        match self {
            Stripe::Api(api) => {
                use stripe_billing::subscription::*;
                if let Some(schedule_id) = &subscription.schedule_id {
                    api.schedule_release(schedule_id).await?;
                }
                let mut item = UpdateSubscriptionItems::new();
                item.id = Some(subscription.item_id.clone());
                item.price = Some(price_id.to_string());
                UpdateSubscription::new(subscription.id.as_str())
                    .items(vec![item])
                    .proration_behavior(UpdateSubscriptionProrationBehavior::AlwaysInvoice)
                    .cancel_at_period_end(false)
                    .send(&api.client)
                    .await
                    .map_err(StripeError::from_api)?;
                Ok(())
            }
            Stripe::Memory(memory) => {
                memory_subscription_change(memory, &subscription.id, |stored| {
                    stored.price_id = price_id.to_string();
                    stored.cancel_at_period_end = false;
                    stored.schedule_id = None;
                    stored.scheduled_price_id = None;
                    stored.scheduled_at = None;
                })
            }
        }
    }

    /// The subscription's price changed when the period paid for ends,
    /// with a subscription schedule: this period at the price it has,
    /// then the new one, every `interval`, and the schedule released.
    pub async fn subscription_price_update_at_period_end(
        &self,
        subscription: &StripeSubscription,
        price_id: &str,
        interval: &str,
    ) -> Result<(), StripeError> {
        match self {
            Stripe::Api(api) => {
                use stripe_billing::subscription_schedule::*;
                let schedule_id = match &subscription.schedule_id {
                    Some(schedule_id) => schedule_id.clone(),
                    None => CreateSubscriptionSchedule::new()
                        .from_subscription(subscription.id.as_str())
                        .send(&api.client)
                        .await
                        .map_err(StripeError::from_api)?
                        .id
                        .as_str()
                        .to_string(),
                };
                let schedule = api.schedule_get(&schedule_id).await?;
                let current = schedule.current_phase.as_ref().ok_or_else(|| {
                    StripeError::Refused(format!("schedule {schedule_id} has no current phase"))
                })?;
                let mut now_item = UpdateSubscriptionSchedulePhasesItems::new();
                now_item.price = Some(subscription.price_id.clone());
                let mut this_period = UpdateSubscriptionSchedulePhases::new(vec![now_item]);
                this_period.start_date = Some(
                    UpdateSubscriptionSchedulePhasesStartDate::Timestamp(current.start_date),
                );
                this_period.end_date = Some(UpdateSubscriptionSchedulePhasesEndDate::Timestamp(
                    current.end_date,
                ));
                let mut next_item = UpdateSubscriptionSchedulePhasesItems::new();
                next_item.price = Some(price_id.to_string());
                let mut next = UpdateSubscriptionSchedulePhases::new(vec![next_item]);
                let interval = match interval {
                    "year" => UpdateSubscriptionSchedulePhasesDurationInterval::Year,
                    _ => UpdateSubscriptionSchedulePhasesDurationInterval::Month,
                };
                next.duration = Some(UpdateSubscriptionSchedulePhasesDuration::new(interval));
                UpdateSubscriptionSchedule::new(schedule_id.as_str())
                    .phases(vec![this_period, next])
                    .end_behavior(stripe_shared::SubscriptionScheduleEndBehavior::Release)
                    .send(&api.client)
                    .await
                    .map_err(StripeError::from_api)?;
                Ok(())
            }
            Stripe::Memory(memory) => {
                memory_subscription_change(memory, &subscription.id, |stored| {
                    stored.schedule_id = Some(format!("sub_sched_{}", stored.id));
                    stored.scheduled_price_id = Some(price_id.to_string());
                    stored.scheduled_at = stored.current_period_end;
                })
            }
        }
    }

    /// A change waiting for the end of the period undone: the plan goes
    /// on at the price it has.
    pub async fn subscription_schedule_release(
        &self,
        subscription: &StripeSubscription,
    ) -> Result<(), StripeError> {
        let Some(schedule_id) = &subscription.schedule_id else {
            return Ok(());
        };
        match self {
            Stripe::Api(api) => api.schedule_release(schedule_id).await,
            Stripe::Memory(memory) => {
                memory_subscription_change(memory, &subscription.id, |stored| {
                    stored.schedule_id = None;
                    stored.scheduled_price_id = None;
                    stored.scheduled_at = None;
                })
            }
        }
    }

    /// Whether the subscription ends when the period paid for does. A
    /// change waiting for then is undone first.
    pub async fn subscription_cancel_at_period_end(
        &self,
        subscription: &StripeSubscription,
        cancel: bool,
    ) -> Result<(), StripeError> {
        if cancel {
            self.subscription_schedule_release(subscription).await?;
        }
        match self {
            Stripe::Api(api) => {
                stripe_billing::subscription::UpdateSubscription::new(subscription.id.as_str())
                    .cancel_at_period_end(cancel)
                    .send(&api.client)
                    .await
                    .map_err(StripeError::from_api)?;
                Ok(())
            }
            Stripe::Memory(memory) => {
                memory_subscription_change(memory, &subscription.id, |stored| {
                    stored.cancel_at_period_end = cancel;
                })
            }
        }
    }

    /// Ends the subscription now, with no refund and no last invoice,
    /// for a deleted account, see `accounts::deletions`.
    pub async fn subscription_cancel(&self, id: &str) -> Result<(), StripeError> {
        match self {
            Stripe::Api(api) => {
                stripe_billing::subscription::CancelSubscription::new(id)
                    .prorate(false)
                    .invoice_now(false)
                    .send(&api.client)
                    .await
                    .map_err(StripeError::from_api)?;
                Ok(())
            }
            Stripe::Memory(memory) => memory_subscription_change(memory, id, |stored| {
                stored.status = "canceled".to_string();
                stored.cancel_at_period_end = false;
                stored.ended_at = Some(chrono::Utc::now().timestamp());
            }),
        }
    }

    /// Deletes the customer, which removes their saved cards; Stripe
    /// keeps their payments and invoices. One already gone is fine.
    pub async fn customer_delete(&self, customer_id: &str) -> Result<(), StripeError> {
        match self {
            Stripe::Api(api) => {
                match stripe_core::customer::DeleteCustomer::new(customer_id)
                    .send(&api.client)
                    .await
                {
                    Ok(_) | Err(stripe::StripeError::Stripe(_, 404)) => Ok(()),
                    Err(error) => Err(StripeError::from_api(error)),
                }
            }
            Stripe::Memory(memory) => {
                lock(memory).customers.retain(|_, id| id != customer_id);
                Ok(())
            }
        }
    }

    /// Stripe's Customer Portal for the customer, for their card and
    /// invoices, coming back to `return_url`: its page.
    pub async fn billing_portal_session_create(
        &self,
        customer_id: &str,
        return_url: &str,
    ) -> Result<String, StripeError> {
        match self {
            Stripe::Api(api) => {
                let session =
                    stripe_billing::billing_portal_session::CreateBillingPortalSession::new()
                        .customer(customer_id)
                        .return_url(return_url)
                        .send(&api.client)
                        .await
                        .map_err(StripeError::from_api)?;
                Ok(session.url)
            }
            Stripe::Memory(_) => Ok(format!(
                "https://billing.stripe.test/{customer_id}?return_url={return_url}"
            )),
        }
    }

    /// The subscription a schedule changes, `sub_...`, if it has one.
    pub async fn subscription_schedule_subscription_id(
        &self,
        schedule_id: &str,
    ) -> Result<Option<String>, StripeError> {
        match self {
            Stripe::Api(api) => {
                let schedule = api.schedule_get(schedule_id).await?;
                Ok(schedule
                    .subscription
                    .as_ref()
                    .map(|subscription| subscription.id().as_str().to_string()))
            }
            Stripe::Memory(memory) => Ok(lock(memory)
                .subscriptions
                .values()
                .find(|subscription| subscription.schedule_id.as_deref() == Some(schedule_id))
                .map(|subscription| subscription.id.clone())),
        }
    }

    pub fn memory_charge_put(&self, charge: StripeCharge) {
        if let Stripe::Memory(memory) = self {
            lock(memory).charges.insert(charge.id.clone(), charge);
        }
    }

    /// A customer for the account, `cus_...`. Asked once per account: a
    /// second ask within a day answers the first's customer.
    pub async fn customer_create(
        &self,
        account_id: &str,
        email: &str,
    ) -> Result<String, StripeError> {
        match self {
            Stripe::Api(api) => {
                let once = IdempotencyKey::new(format!("customer-{account_id}"))
                    .map_err(|error| StripeError::Refused(error.to_string()))?;
                let customer = stripe_core::customer::CreateCustomer::new()
                    .email(email)
                    .metadata([("account_id".to_string(), account_id.to_string())])
                    .customize()
                    .request_strategy(RequestStrategy::Idempotent(once))
                    .send(&api.client)
                    .await
                    .map_err(StripeError::from_api)?;
                Ok(customer.id.as_str().to_string())
            }
            Stripe::Memory(memory) => {
                let mut memory = lock(memory);
                let count = memory.customers.len();
                let id = memory
                    .customers
                    .entry(email.to_string())
                    .or_insert_with(|| format!("cus_test_{count}"));
                Ok(id.clone())
            }
        }
    }

    pub async fn checkout_session_create(
        &self,
        new: &StripeCheckoutSessionNew,
    ) -> Result<StripeCheckoutSession, StripeError> {
        match self {
            Stripe::Api(api) => {
                use stripe_checkout::checkout_session::*;
                let mut line_item = CreateCheckoutSessionLineItems::new();
                line_item.price = Some(new.price_id.clone());
                line_item.quantity = Some(1);
                let mut request = CreateCheckoutSession::new()
                    .customer(new.customer_id.clone())
                    .line_items(vec![line_item])
                    .client_reference_id(new.client_reference_id.clone())
                    .success_url(new.success_url.clone())
                    .cancel_url(new.cancel_url.clone())
                    .expand(vec!["payment_intent".to_string()]);
                request = match new.mode {
                    CheckoutMode::Payment => {
                        request.mode(stripe_shared::CheckoutSessionMode::Payment)
                    }
                    CheckoutMode::Subscription => {
                        let mut subscription = CreateCheckoutSessionSubscriptionData::new();
                        subscription.trial_period_days = new.free_days;
                        request
                            .mode(stripe_shared::CheckoutSessionMode::Subscription)
                            .subscription_data(subscription)
                    }
                };
                request = match new.tax {
                    StripeTax::ManagedPayments => {
                        let mut managed = CreateCheckoutSessionManagedPayments::new();
                        managed.enabled = Some(true);
                        request.managed_payments(managed)
                    }
                    StripeTax::StripeTax => {
                        let mut update = CreateCheckoutSessionCustomerUpdate::new();
                        update.address = Some(CreateCheckoutSessionCustomerUpdateAddress::Auto);
                        update.name = Some(CreateCheckoutSessionCustomerUpdateName::Auto);
                        request
                            .automatic_tax(CreateCheckoutSessionAutomaticTax::new(true))
                            .customer_update(update)
                    }
                };
                let session = request
                    .send(&api.client)
                    .await
                    .map_err(StripeError::from_api)?;
                Ok(checkout_session_from(&session))
            }
            Stripe::Memory(memory) => {
                let mut memory = lock(memory);
                let id = format!("cs_test_{}", memory.checkout_sessions_asked.len());
                let session = StripeCheckoutSession {
                    id: id.clone(),
                    url: Some(format!("https://checkout.stripe.test/{id}")),
                    client_reference_id: Some(new.client_reference_id.clone()),
                    status: StripeCheckoutSessionStatus::Open,
                    payment_status: StripePaymentStatus::Unpaid,
                    payment_intent_id: None,
                    subscription_id: None,
                    amount_total: None,
                    currency: None,
                };
                memory.checkout_sessions_asked.push(new.clone());
                memory.checkout_sessions.insert(id, session.clone());
                Ok(session)
            }
        }
    }

    pub async fn checkout_session_get(
        &self,
        id: &str,
    ) -> Result<StripeCheckoutSession, StripeError> {
        match self {
            Stripe::Api(api) => {
                let session = stripe_checkout::checkout_session::RetrieveCheckoutSession::new(id)
                    .expand(vec!["payment_intent".to_string()])
                    .send(&api.client)
                    .await
                    .map_err(|error| StripeError::from_api_for("checkout session", id, error))?;
                Ok(checkout_session_from(&session))
            }
            Stripe::Memory(memory) => {
                lock(memory)
                    .checkout_sessions
                    .get(id)
                    .cloned()
                    .ok_or_else(|| StripeError::NotFound {
                        object: "checkout session",
                        id: id.to_string(),
                    })
            }
        }
    }

    pub async fn charge_get(&self, id: &str) -> Result<StripeCharge, StripeError> {
        match self {
            Stripe::Api(api) => {
                let charge = stripe_core::charge::RetrieveCharge::new(id)
                    .send(&api.client)
                    .await
                    .map_err(|error| StripeError::from_api_for("charge", id, error))?;
                Ok(StripeCharge {
                    id: charge.id.as_str().to_string(),
                    payment_intent_id: charge
                        .payment_intent
                        .as_ref()
                        .map(|intent| intent.id().as_str().to_string()),
                    refunded: charge.refunded,
                })
            }
            Stripe::Memory(memory) => {
                lock(memory)
                    .charges
                    .get(id)
                    .cloned()
                    .ok_or_else(|| StripeError::NotFound {
                        object: "charge",
                        id: id.to_string(),
                    })
            }
        }
    }
}

impl StripeApi {
    async fn schedule_get(
        &self,
        id: &str,
    ) -> Result<stripe_shared::SubscriptionSchedule, StripeError> {
        stripe_billing::subscription_schedule::RetrieveSubscriptionSchedule::new(id)
            .send(&self.client)
            .await
            .map_err(|error| StripeError::from_api_for("subscription schedule", id, error))
    }

    async fn schedule_release(&self, id: &str) -> Result<(), StripeError> {
        stripe_billing::subscription_schedule::ReleaseSubscriptionSchedule::new(id)
            .send(&self.client)
            .await
            .map_err(StripeError::from_api)?;
        Ok(())
    }
}

/// Changes the subscription `Memory` keeps, as Stripe would.
fn memory_subscription_change(
    memory: &Mutex<StripeMemory>,
    id: &str,
    change: impl FnOnce(&mut StripeSubscription),
) -> Result<(), StripeError> {
    let mut memory = lock(memory);
    let stored = memory
        .subscriptions
        .get_mut(id)
        .ok_or_else(|| StripeError::NotFound {
            object: "subscription",
            id: id.to_string(),
        })?;
    change(stored);
    Ok(())
}

fn lock(memory: &Mutex<StripeMemory>) -> std::sync::MutexGuard<'_, StripeMemory> {
    memory.lock().unwrap_or_else(PoisonError::into_inner)
}

impl StripeError {
    fn from_api(error: stripe::StripeError) -> StripeError {
        match error {
            stripe::StripeError::Stripe(errors, status) => StripeError::Refused(format!(
                "{status} {}",
                errors.message.as_deref().unwrap_or("no message")
            )),
            other => StripeError::Transport(other.to_string()),
        }
    }

    /// A 404 as `NotFound` for the object asked for.
    fn from_api_for(object: &'static str, id: &str, error: stripe::StripeError) -> StripeError {
        if let stripe::StripeError::Stripe(_, 404) = error {
            return StripeError::NotFound {
                object,
                id: id.to_string(),
            };
        }
        StripeError::from_api(error)
    }
}

fn checkout_session_from(session: &stripe_shared::CheckoutSession) -> StripeCheckoutSession {
    use stripe_shared::{CheckoutSessionPaymentStatus, CheckoutSessionStatus, PaymentIntentStatus};
    let status = match session.status {
        Some(CheckoutSessionStatus::Complete) => StripeCheckoutSessionStatus::Complete,
        Some(CheckoutSessionStatus::Expired) => StripeCheckoutSessionStatus::Expired,
        _ => StripeCheckoutSessionStatus::Open,
    };
    let intent_status = session
        .payment_intent
        .as_ref()
        .and_then(|intent| match intent {
            stripe_types::Expandable::Object(intent) => Some(intent.status.clone()),
            stripe_types::Expandable::Id(_) => None,
        });
    let payment_status = match (&session.payment_status, intent_status) {
        (
            CheckoutSessionPaymentStatus::Paid | CheckoutSessionPaymentStatus::NoPaymentRequired,
            _,
        ) => StripePaymentStatus::Paid,
        (_, Some(PaymentIntentStatus::Processing)) => StripePaymentStatus::Processing,
        (_, Some(PaymentIntentStatus::Canceled)) => StripePaymentStatus::Failed,
        (_, Some(PaymentIntentStatus::RequiresPaymentMethod))
            if status == StripeCheckoutSessionStatus::Complete =>
        {
            StripePaymentStatus::Failed
        }
        _ => StripePaymentStatus::Unpaid,
    };
    StripeCheckoutSession {
        id: session.id.as_str().to_string(),
        url: session.url.clone(),
        client_reference_id: session.client_reference_id.clone(),
        status,
        payment_status,
        payment_intent_id: session
            .payment_intent
            .as_ref()
            .map(|intent| intent.id().as_str().to_string()),
        subscription_id: session
            .subscription
            .as_ref()
            .map(|subscription| subscription.id().as_str().to_string()),
        amount_total: session.amount_total,
        currency: session.currency.as_ref().map(ToString::to_string),
    }
}

/// The subscription, and the change its schedule, if any, makes when the
/// current phase ends: the next phase's price, when it is another.
fn subscription_from(
    subscription: &stripe_shared::Subscription,
    schedule: Option<&stripe_shared::SubscriptionSchedule>,
) -> Result<StripeSubscription, StripeError> {
    let item = subscription.items.data.first().ok_or_else(|| {
        StripeError::Refused(format!(
            "subscription {} has no price",
            subscription.id.as_str()
        ))
    })?;
    let schedule = schedule
        .filter(|schedule| schedule.status == stripe_shared::SubscriptionScheduleStatus::Active);
    let current_end = schedule
        .and_then(|schedule| schedule.current_phase.as_ref())
        .map(|phase| phase.end_date);
    let next_phase = schedule.and_then(|schedule| {
        schedule
            .phases
            .iter()
            .find(|phase| Some(phase.start_date) == current_end)
    });
    let scheduled_price_id = next_phase
        .and_then(|phase| phase.items.first())
        .map(|item| item.price.id().as_str().to_string())
        .filter(|price_id| price_id != item.price.id.as_str());
    Ok(StripeSubscription {
        id: subscription.id.as_str().to_string(),
        customer_id: subscription.customer.id().as_str().to_string(),
        item_id: item.id.as_str().to_string(),
        price_id: item.price.id.as_str().to_string(),
        status: subscription.status.as_str().to_string(),
        current_period_end: Some(item.current_period_end),
        cancel_at_period_end: subscription.cancel_at_period_end || subscription.cancel_at.is_some(),
        ended_at: subscription.ended_at,
        schedule_id: schedule.map(|schedule| schedule.id.as_str().to_string()),
        scheduled_at: scheduled_price_id
            .as_ref()
            .and(next_phase.map(|phase| phase.start_date)),
        scheduled_price_id,
    })
}

fn product_from(product: &stripe_shared::Product) -> StripeProduct {
    StripeProduct {
        id: product.id.as_str().to_string(),
        name: product.name.clone(),
        description: product.description.clone(),
        active: product.active,
    }
}

fn price_from(price: &stripe_shared::Price) -> StripePrice {
    let interval = price
        .recurring
        .as_ref()
        .filter(|recurring| recurring.interval_count == 1)
        .map(|recurring| recurring.interval.as_str().to_string());
    StripePrice {
        id: price.id.as_str().to_string(),
        product_id: price.product.id().as_str().to_string(),
        lookup_key: price.lookup_key.clone(),
        amount: price.unit_amount,
        currency: price.currency.to_string(),
        interval,
        // Every few months or weeks is no interval the catalog sells
        active: price.active
            && price
                .recurring
                .as_ref()
                .is_none_or(|recurring| recurring.interval_count == 1),
    }
}
