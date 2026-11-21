//! Buying one thing: a checkout, one row of `accounts_checkouts`, made
//! with a Stripe Checkout Session, where the person pays, then followed
//! until it is paid, expired or failed. A paid one-time checkout is a
//! purchase, `purchases`; a paid plan's is a subscription,
//! `subscriptions`. Server only, and only for someone with an
//! account, never a trial user. Stripe's return to the server fetches
//! the session at once, `get`, so the page does not wait for the
//! webhook, which applies the same session, `apply_session_in`. See
//! `docs/design/purchasing.md`, P2 and P3.

use crate::data::accounts_checkouts as accounts_checkouts_records;
use crate::data::accounts_products as accounts_products_records;
use crate::data::accounts_purchases as accounts_purchases_records;
use crate::data::accounts_users as accounts_users_records;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::accounts::online::ReturnTo;
use crate::accounts::prices;
use crate::accounts::products::{self, ProductError, ProductKind};
use crate::accounts::purchases::{self, Purchase, Store};
use crate::accounts::{plans, subscriptions};
use crate::app::users::User;
use crate::database::Transaction;
use crate::deployment::{Deployment, Role, now};
use crate::services::stripe::{
    CheckoutMode, Stripe, StripeCheckoutSession, StripeCheckoutSessionNew,
    StripeCheckoutSessionStatus, StripeError, StripePaymentStatus, StripeSubscription,
};

/// A checkout status accepted by provider-session processing.
pub(crate) struct AcceptedStatus(CheckoutStatus);

impl AcceptedStatus {
    /// Returns the accepted provider status.
    pub(crate) fn value(&self) -> CheckoutStatus {
        self.0
    }
}

/// A record accepted by the checkouts workflow.
pub(crate) struct AcceptedCheckout<'a>(&'a Checkout);

impl AcceptedCheckout<'_> {
    /// Borrows the accepted record for persistence.
    pub(crate) fn row(&self) -> &Checkout {
        self.0
    }
}

pub const TABLE: &str = "accounts_checkouts";

/// The page Stripe comes back to for the app: open to anyone, as the
/// browser that paid is not signed in, saying to go back to the app.
pub const BACK_TO_APP_PATH: &str = "/accounts/back-to-app";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckoutStatus {
    /// On Stripe's page, not paid yet.
    Open,
    /// Paid with a method that settles later.
    Processing,
    Paid,
    /// Left unpaid until Stripe closed it.
    Expired,
    /// A payment that settles later and did not.
    Failed,
}

impl CheckoutStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            CheckoutStatus::Open => "open",
            CheckoutStatus::Processing => "processing",
            CheckoutStatus::Paid => "paid",
            CheckoutStatus::Expired => "expired",
            CheckoutStatus::Failed => "failed",
        }
    }

    pub fn from_name(name: &str) -> Option<CheckoutStatus> {
        match name {
            "open" => Some(CheckoutStatus::Open),
            "processing" => Some(CheckoutStatus::Processing),
            "paid" => Some(CheckoutStatus::Paid),
            "expired" => Some(CheckoutStatus::Expired),
            "failed" => Some(CheckoutStatus::Failed),
            _ => None,
        }
    }

    /// Whether Stripe may still change it.
    fn is_pending(self) -> bool {
        matches!(self, CheckoutStatus::Open | CheckoutStatus::Processing)
    }

    fn of(session: &StripeCheckoutSession) -> CheckoutStatus {
        match (session.status, session.payment_status) {
            (_, StripePaymentStatus::Paid) => CheckoutStatus::Paid,
            (_, StripePaymentStatus::Failed) => CheckoutStatus::Failed,
            (_, StripePaymentStatus::Processing) => CheckoutStatus::Processing,
            (StripeCheckoutSessionStatus::Expired, _) => CheckoutStatus::Expired,
            (StripeCheckoutSessionStatus::Complete, _) => CheckoutStatus::Processing,
            (StripeCheckoutSessionStatus::Open, _) => CheckoutStatus::Open,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checkout {
    pub id: Uuid,
    pub user_id: Uuid,
    pub price_id: Uuid,
    /// `cs_...`
    pub stripe_session_id: String,
    pub status: CheckoutStatus,
    pub inserted_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A checkout just made, and Stripe's page to send the person to.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckoutCreated {
    pub checkout: Checkout,
    pub url: String,
}

/// A checkout as its page shows it: what is being bought, and the
/// purchase once paid.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckoutShown {
    #[serde(flatten)]
    pub checkout: Checkout,
    pub product_id: Uuid,
    pub product_name: String,
    pub kind: ProductKind,
    pub purchase_id: Option<Uuid>,
}

#[derive(Debug, Error)]
pub enum CheckoutError {
    #[error("only the server takes payments")]
    ServerOnly,
    #[error("Buying needs an account: sign up first")]
    NeedsAccount,
    #[error("No checkout {id} of yours")]
    NotFound { id: Uuid },
    #[error("That price is not for sale")]
    PriceNotForSale,
    #[error("You have this already")]
    AlreadyOwned,
    #[error("You have a plan already")]
    AlreadySubscribed,
    #[error(transparent)]
    Stripe(#[from] StripeError),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl From<plans::PlanError> for CheckoutError {
    fn from(error: plans::PlanError) -> CheckoutError {
        match error {
            plans::PlanError::ServerOnly => CheckoutError::ServerOnly,
            plans::PlanError::Database(error) => CheckoutError::Database(error),
        }
    }
}

impl From<ProductError> for CheckoutError {
    fn from(error: ProductError) -> CheckoutError {
        match error {
            ProductError::NotFound { .. } => CheckoutError::PriceNotForSale,
            ProductError::ServerOnly => CheckoutError::ServerOnly,
            ProductError::Stripe(error) => CheckoutError::Stripe(error),
            ProductError::Database(error) => CheckoutError::Database(error),
        }
    }
}

/// A checkout for the price, with a Checkout Session on Stripe that
/// comes back to `{server_url}/accounts/checkouts/{id}` when paid and
/// to the product's page when the person goes back; for the app, to
/// `{server_url}/accounts/back-to-app` either way.
pub async fn create(
    deployment: &Deployment,
    stripe: &Stripe,
    user: &User,
    price_id: Uuid,
    server_url: &str,
    return_to: ReturnTo,
) -> Result<CheckoutCreated, CheckoutError> {
    if deployment.role != Role::Server {
        return Err(CheckoutError::ServerOnly);
    }
    let mut tx = deployment.begin().await?;
    let account = accounts_users_records::lifecycle::get(&mut tx, user.id)
        .await?
        .ok_or(CheckoutError::NeedsAccount)?;
    if account.trial_expires_at.is_some() || account.confirmed_at.is_none() {
        return Err(CheckoutError::NeedsAccount);
    }
    let price = prices::get_in(&mut tx, price_id)
        .await?
        .ok_or(CheckoutError::PriceNotForSale)?;
    let owned = purchases::owns_in(&mut tx, user.id, price.product_id).await?;
    let customer_id =
        accounts_users_records::billing::get_stripe_customer_id(&mut tx, user.id).await?;
    tx.commit().await?;
    let product = products::get_for_sale(deployment, price.product_id).await?;
    if !product.prices.iter().any(|sold| sold.id == price_id) {
        return Err(CheckoutError::PriceNotForSale);
    }
    if product.kind == ProductKind::OneTime && owned && !product.is_repeatable {
        return Err(CheckoutError::AlreadyOwned);
    }
    if product.kind == ProductKind::Plan && plans::current(deployment, user).await?.is_some() {
        return Err(CheckoutError::AlreadySubscribed);
    }

    let customer_id = match customer_id {
        Some(customer_id) => customer_id,
        None => customer_for(deployment, stripe, user.id, &account.email).await?,
    };
    let id = Uuid::now_v7();
    let session = stripe
        .checkout_session_create(&StripeCheckoutSessionNew {
            customer_id,
            price_id: price.stripe_id.clone(),
            mode: match product.kind {
                ProductKind::Plan => CheckoutMode::Subscription,
                ProductKind::OneTime => CheckoutMode::Payment,
            },
            client_reference_id: id.to_string(),
            success_url: match return_to {
                ReturnTo::Web => format!("{server_url}/accounts/checkouts/{id}"),
                ReturnTo::App => format!("{server_url}{BACK_TO_APP_PATH}"),
            },
            cancel_url: match return_to {
                ReturnTo::Web => format!("{server_url}/accounts/products/{}", product.id),
                ReturnTo::App => format!("{server_url}{BACK_TO_APP_PATH}"),
            },
            free_days: product.free_days,
            tax: stripe.tax(),
        })
        .await?;
    let url = session
        .url
        .clone()
        .ok_or_else(|| StripeError::Refused("the checkout session has no page".to_string()))?;
    let now = now();
    let checkout = Checkout {
        id,
        user_id: user.id,
        price_id,
        stripe_session_id: session.id,
        status: CheckoutStatus::Open,
        inserted_at: now,
        updated_at: now,
    };
    let mut tx = deployment.begin().await?;
    accounts_checkouts_records::create(&mut tx, AcceptedCheckout(&checkout)).await?;
    tx.commit().await?;
    Ok(CheckoutCreated { checkout, url })
}

/// One of the user's checkouts, asking Stripe first while it may still
/// change, so the return from Stripe's page shows it paid.
pub async fn get(
    deployment: &Deployment,
    stripe: &Stripe,
    user: &User,
    id: Uuid,
) -> Result<CheckoutShown, CheckoutError> {
    if deployment.role != Role::Server {
        return Err(CheckoutError::ServerOnly);
    }
    let checkout = checkout_of(deployment, user, id).await?;
    if checkout.status.is_pending() {
        let session = stripe
            .checkout_session_get(&checkout.stripe_session_id)
            .await?;
        let subscription = match &session.subscription_id {
            Some(id) => Some(stripe.subscription_get(id).await?),
            None => None,
        };
        let mut tx = deployment.begin().await?;
        apply_session_in(&mut tx, &session, subscription.as_ref(), now()).await?;
        tx.commit().await?;
    }
    let checkout = checkout_of(deployment, user, id).await?;
    let mut tx = deployment.begin().await?;
    let price = prices::get_in(&mut tx, checkout.price_id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)?;
    let product = accounts_products_records::get(&mut tx, price.product_id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)?;
    let purchase_id =
        accounts_purchases_records::processing::get_by_checkout_id(&mut tx, checkout.id).await?;
    tx.commit().await?;
    let kind = match price.interval {
        Some(_) => ProductKind::Plan,
        None => ProductKind::OneTime,
    };
    Ok(CheckoutShown {
        checkout,
        product_id: product.id,
        product_name: product.name,
        kind,
        purchase_id,
    })
}

/// The session as Stripe has it now, on the checkout that made it: its
/// status, a purchase once a one-time checkout is paid, and a plan's
/// subscription, which Stripe answers beside it. A session no checkout
/// here made is left alone.
pub(crate) async fn apply_session_in(
    tx: &mut Transaction,
    session: &StripeCheckoutSession,
    subscription: Option<&StripeSubscription>,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    let Some(checkout) =
        accounts_checkouts_records::processing::get_by_stripe_session_id(&mut *tx, &session.id)
            .await?
    else {
        return Ok(());
    };
    let status = CheckoutStatus::of(session);
    if status != checkout.status {
        accounts_checkouts_records::update(&mut *tx, checkout.id, AcceptedStatus(status), now)
            .await?;
    }
    if let Some(subscription) = subscription {
        subscriptions::upsert_from_stripe_in(tx, subscription, now).await?;
    }
    let (CheckoutStatus::Paid, Some(payment_intent_id)) = (status, &session.payment_intent_id)
    else {
        return Ok(());
    };
    let price = prices::get_in(&mut *tx, checkout.price_id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)?;
    let purchase = Purchase {
        id: Uuid::now_v7(),
        user_id: checkout.user_id,
        product_id: price.product_id,
        price_id: price.id,
        checkout_id: Some(checkout.id),
        store: Store::Stripe,
        store_id: payment_intent_id.clone(),
        amount: session.amount_total.unwrap_or(price.amount),
        currency: session.currency.clone().unwrap_or(price.currency),
        refunded_at: None,
        inserted_at: now,
        updated_at: now,
    };
    purchases::insert_if_missing_in(tx, &purchase).await
}

async fn checkout_of(
    deployment: &Deployment,
    user: &User,
    id: Uuid,
) -> Result<Checkout, CheckoutError> {
    let mut tx = deployment.begin().await?;
    let checkout = accounts_checkouts_records::get(&mut tx, user.id, id).await?;
    tx.commit().await?;
    checkout.ok_or(CheckoutError::NotFound { id })
}

/// The account's Stripe customer, made now. Two checkouts at once may
/// both make one; the first kept is the account's.
async fn customer_for(
    deployment: &Deployment,
    stripe: &Stripe,
    account_id: Uuid,
    email: &str,
) -> Result<String, CheckoutError> {
    let made = stripe
        .customer_create(&account_id.to_string(), email)
        .await?;
    let mut tx = deployment.begin().await?;
    accounts_users_records::billing::set_stripe_customer_id_if_missing(&mut tx, account_id, &made)
        .await?;
    let kept = accounts_users_records::billing::get_stripe_customer_id(&mut tx, account_id).await?;
    tx.commit().await?;
    Ok(kept.unwrap_or(made))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts::stripe_events;
    use crate::accounts::trial_users;
    use crate::app::users;
    use crate::database::with_each_database;
    use crate::services::stripe::{StripeCharge, StripePrice, StripeProduct, StripeTax};

    const SERVER_URL: &str = "http://server.test";

    /// Stripe with the license, bought once, and credits, bought again
    /// and again, copied to the catalog.
    async fn catalog(deployment: &Deployment) -> Stripe {
        let stripe = Stripe::memory();
        for (product_id, name, price_id, lookup_key) in [
            ("prod_license", "License", "price_license", "license"),
            ("prod_credits", "Credits", "price_credits", "credits"),
        ] {
            stripe.memory_product_put(StripeProduct {
                id: product_id.to_string(),
                name: name.to_string(),
                description: None,
                active: true,
            });
            stripe.memory_price_put(StripePrice {
                id: price_id.to_string(),
                product_id: product_id.to_string(),
                lookup_key: Some(lookup_key.to_string()),
                amount: Some(4900),
                currency: "usd".to_string(),
                interval: None,
                active: true,
            });
        }
        products::sync(deployment, &stripe).await.expect("synced");
        stripe
    }

    async fn price_named(deployment: &Deployment, name: &str) -> Uuid {
        products::list(deployment)
            .await
            .expect("the catalog")
            .into_iter()
            .find(|product| product.name == name)
            .expect("the product")
            .prices[0]
            .id
    }

    /// The session paid, as Stripe leaves it after a card payment.
    fn pay(stripe: &Stripe, session_id: &str, payment_intent_id: &str) {
        stripe.memory_checkout_session_put(StripeCheckoutSession {
            id: session_id.to_string(),
            url: None,
            client_reference_id: None,
            status: StripeCheckoutSessionStatus::Complete,
            payment_status: StripePaymentStatus::Paid,
            payment_intent_id: Some(payment_intent_id.to_string()),
            subscription_id: None,
            amount_total: Some(5390),
            currency: Some("eur".to_string()),
        });
    }

    #[tokio::test]
    async fn checkouts_create_then_get_buys_the_license_once() {
        with_each_database(|database| async move {
            let deployment = Deployment::new(database, Role::Server);
            let stripe = catalog(&deployment).await;
            let alice = users::create(&deployment, "alice@example.com", "Alice")
                .await
                .expect("a user");
            let license = price_named(&deployment, "License").await;

            let created = create(
                &deployment,
                &stripe,
                &alice,
                license,
                SERVER_URL,
                ReturnTo::Web,
            )
            .await
            .expect("a checkout");
            assert_eq!(created.checkout.status, CheckoutStatus::Open);
            assert!(created.url.starts_with("https://checkout.stripe.test/"));
            if let Stripe::Memory(memory) = &stripe {
                let memory = memory.lock().expect("the memory");
                let asked = &memory.checkout_sessions_asked[0];
                assert_eq!(asked.mode, CheckoutMode::Payment);
                assert_eq!(asked.price_id, "price_license");
                assert_eq!(asked.tax, StripeTax::ManagedPayments);
                assert_eq!(asked.client_reference_id, created.checkout.id.to_string());
                assert_eq!(
                    asked.success_url,
                    format!("{SERVER_URL}/accounts/checkouts/{}", created.checkout.id)
                );
                assert_eq!(memory.customers.len(), 1);
            }
            let id = created.checkout.id;
            let shown = get(&deployment, &stripe, &alice, id).await.expect("open");
            assert_eq!(shown.checkout.status, CheckoutStatus::Open);
            assert_eq!(shown.product_name, "License");
            assert_eq!(shown.purchase_id, None);

            pay(&stripe, &created.checkout.stripe_session_id, "pi_1");
            let shown = get(&deployment, &stripe, &alice, id).await.expect("paid");
            assert_eq!(shown.checkout.status, CheckoutStatus::Paid);
            let purchase_id = shown.purchase_id.expect("a purchase");
            let purchase = purchases::get(&deployment, &alice, purchase_id)
                .await
                .expect("the purchase");
            assert_eq!(purchase.product_name, "License");
            assert_eq!(purchase.purchase.amount, 5390);
            assert_eq!(purchase.purchase.currency, "eur");
            assert_eq!(purchase.purchase.store_id, "pi_1");
            let again = get(&deployment, &stripe, &alice, id)
                .await
                .expect("still paid");
            assert_eq!(again.purchase_id, Some(purchase_id));

            let product_id = shown.product_id;
            assert!(
                products::get(&deployment, &alice, product_id)
                    .await
                    .expect("shown")
                    .owned
            );
            let error = create(
                &deployment,
                &stripe,
                &alice,
                license,
                SERVER_URL,
                ReturnTo::Web,
            )
            .await
            .expect_err("owned");
            assert!(matches!(error, CheckoutError::AlreadyOwned));

            let bob = users::create(&deployment, "bob@example.com", "Bob")
                .await
                .expect("a user");
            let error = get(&deployment, &stripe, &bob, id)
                .await
                .expect_err("not his");
            assert!(matches!(error, CheckoutError::NotFound { .. }));
        })
        .await;
    }

    #[tokio::test]
    async fn checkouts_create_buys_credits_again_and_keeps_one_customer() {
        with_each_database(|database| async move {
            let deployment = Deployment::new(database, Role::Server);
            let stripe = catalog(&deployment).await;
            let alice = users::create(&deployment, "alice@example.com", "Alice")
                .await
                .expect("a user");
            let credits = price_named(&deployment, "Credits").await;
            for payment in ["pi_1", "pi_2"] {
                let created = create(
                    &deployment,
                    &stripe,
                    &alice,
                    credits,
                    SERVER_URL,
                    ReturnTo::Web,
                )
                .await
                .expect("a checkout");
                pay(&stripe, &created.checkout.stripe_session_id, payment);
                get(&deployment, &stripe, &alice, created.checkout.id)
                    .await
                    .expect("paid");
            }
            let bought = purchases::list(&deployment, &alice)
                .await
                .expect("purchases");
            assert_eq!(bought.len(), 2);
            if let Stripe::Memory(memory) = &stripe {
                let memory = memory.lock().expect("the memory");
                let customers: Vec<&String> = memory
                    .checkout_sessions_asked
                    .iter()
                    .map(|asked| &asked.customer_id)
                    .collect();
                assert_eq!(customers[0], customers[1]);
            }
        })
        .await;
    }

    #[tokio::test]
    async fn checkouts_create_for_the_app_comes_back_to_the_page_for_it() {
        with_each_database(|database| async move {
            let deployment = Deployment::new(database, Role::Server);
            let stripe = catalog(&deployment).await;
            let alice = users::create(&deployment, "alice@example.com", "Alice")
                .await
                .expect("a user");
            let license = price_named(&deployment, "License").await;
            create(
                &deployment,
                &stripe,
                &alice,
                license,
                SERVER_URL,
                ReturnTo::App,
            )
            .await
            .expect("a checkout");
            if let Stripe::Memory(memory) = &stripe {
                let memory = memory.lock().expect("the memory");
                let asked = &memory.checkout_sessions_asked[0];
                let back = format!("{SERVER_URL}{BACK_TO_APP_PATH}");
                assert_eq!((&asked.success_url, &asked.cancel_url), (&back, &back));
            }
        })
        .await;
    }

    #[tokio::test]
    async fn checkouts_create_needs_an_account_and_a_price_for_sale() {
        with_each_database(|database| async move {
            let deployment = Deployment::new(database, Role::Server);
            let stripe = catalog(&deployment).await;
            let license = price_named(&deployment, "License").await;
            let trial = trial_users::create(&deployment, 5)
                .await
                .expect("a trial user");
            let error = create(
                &deployment,
                &stripe,
                &trial.user,
                license,
                SERVER_URL,
                ReturnTo::Web,
            )
            .await
            .expect_err("a trial user");
            assert!(matches!(error, CheckoutError::NeedsAccount));

            let alice = users::create(&deployment, "alice@example.com", "Alice")
                .await
                .expect("a user");
            let error = create(
                &deployment,
                &stripe,
                &alice,
                Uuid::now_v7(),
                SERVER_URL,
                ReturnTo::Web,
            )
            .await
            .expect_err("no such price");
            assert!(matches!(error, CheckoutError::PriceNotForSale));
        })
        .await;
    }

    #[tokio::test]
    async fn checkouts_follow_the_webhook_and_a_refund_takes_the_license_back() {
        with_each_database(|database| async move {
            let deployment = Deployment::new(database, Role::Server);
            let stripe = catalog(&deployment).await;
            let alice = users::create(&deployment, "alice@example.com", "Alice")
                .await
                .expect("a user");
            let license = price_named(&deployment, "License").await;
            let created = create(
                &deployment,
                &stripe,
                &alice,
                license,
                SERVER_URL,
                ReturnTo::Web,
            )
            .await
            .expect("a checkout");
            let session_id = created.checkout.stripe_session_id.clone();
            pay(&stripe, &session_id, "pi_1");
            deliver(
                &deployment,
                &stripe,
                "evt_1",
                "checkout.session.completed",
                &session_id,
            )
            .await;
            let bought = purchases::list(&deployment, &alice)
                .await
                .expect("purchases");
            assert_eq!(bought.len(), 1);
            let product_id = bought[0].purchase.product_id;

            stripe.memory_charge_put(StripeCharge {
                id: "ch_1".to_string(),
                payment_intent_id: Some("pi_1".to_string()),
                refunded: true,
            });
            deliver(&deployment, &stripe, "evt_2", "charge.refunded", "ch_1").await;
            let bought = purchases::list(&deployment, &alice)
                .await
                .expect("purchases");
            assert!(bought[0].purchase.refunded_at.is_some());
            assert!(
                !products::get(&deployment, &alice, product_id)
                    .await
                    .expect("shown")
                    .owned
            );
            create(
                &deployment,
                &stripe,
                &alice,
                license,
                SERVER_URL,
                ReturnTo::Web,
            )
            .await
            .expect("refunded, it can be bought again");
        })
        .await;
    }

    async fn deliver(
        deployment: &Deployment,
        stripe: &Stripe,
        id: &str,
        event_type: &str,
        object_id: &str,
    ) {
        let body = serde_json::json!({
            "id": id,
            "object": "event",
            "api_version": "2026-09-30.endive",
            "created": now().timestamp(),
            "data": { "object": { "id": object_id, "object": "test_object" } },
            "livemode": false,
            "pending_webhooks": 1,
            "request": { "id": null, "idempotency_key": null },
            "type": event_type,
        })
        .to_string();
        let secret = secrecy::SecretString::from("whsec_test");
        let signature = stripe_webhook::Webhook::generate_test_header(&body, "whsec_test", None);
        stripe_events::create(deployment, stripe, &secret, &body, &signature)
            .await
            .expect("handled");
    }

    #[test]
    fn checkout_status_of_a_session() {
        let session = |status, payment_status| StripeCheckoutSession {
            id: "cs_1".to_string(),
            url: None,
            client_reference_id: None,
            status,
            payment_status,
            payment_intent_id: None,
            subscription_id: None,
            amount_total: None,
            currency: None,
        };
        use StripeCheckoutSessionStatus as S;
        use StripePaymentStatus as P;
        for (status, payment_status, expected) in [
            (S::Open, P::Unpaid, CheckoutStatus::Open),
            (S::Complete, P::Paid, CheckoutStatus::Paid),
            (S::Complete, P::Processing, CheckoutStatus::Processing),
            (S::Complete, P::Failed, CheckoutStatus::Failed),
            (S::Expired, P::Unpaid, CheckoutStatus::Expired),
        ] {
            assert_eq!(
                CheckoutStatus::of(&session(status, payment_status)),
                expected
            );
        }
    }
}
