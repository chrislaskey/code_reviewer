//! The events Stripe sends to the webhook, one row of
//! `accounts_stripe_events` per event handled. Server only. Stripe may
//! send an event more than once, so a repeat is recognized by its id and
//! skipped; the row is written in the same transaction as what the event
//! changes, so an event that fails is not recorded and Stripe sends it
//! again. An event only says what changed: the object it names is
//! fetched from Stripe and copied as it is now, so events out of order
//! never leave an older state. See `docs/design/purchasing.md`.

use crate::data::accounts_stripe_events as accounts_stripe_events_records;
use chrono::{DateTime, Utc};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use stripe_webhook::{Webhook, WebhookError};
use thiserror::Error;

use crate::accounts::purchases::{self, Store};
use crate::accounts::stripe::{
    Stripe, StripeCharge, StripeCheckoutSession, StripeError, StripePrice, StripeProduct,
    StripeSubscription,
};
use crate::accounts::{checkouts, prices, products, subscriptions};
use crate::database::Transaction;
use crate::deployment::{Deployment, Role, now};

/// A record accepted by the stripe events workflow.
pub(crate) struct AcceptedStripeEvent<'a>(&'a StripeEvent);

impl AcceptedStripeEvent<'_> {
    /// Borrows the accepted record for persistence.
    pub(crate) fn row(&self) -> &StripeEvent {
        self.0
    }
}

pub const TABLE: &str = "accounts_stripe_events";

/// An event handled, by Stripe's id, `evt_...`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StripeEvent {
    pub id: String,
    /// Stripe's name for it, `checkout.session.completed`.
    pub event_type: String,
    pub handled_at: DateTime<Utc>,
}

/// What an event's body is read for.
#[derive(Deserialize)]
struct EventBody {
    id: String,
    #[serde(rename = "type")]
    event_type: String,
    data: EventData,
}

#[derive(Deserialize)]
struct EventData {
    object: EventObject,
}

#[derive(Deserialize)]
struct EventObject {
    id: Option<String>,
}

/// What `create` did with a delivery.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StripeEventCreated {
    /// The first delivery of the event: handled now.
    New(StripeEvent),
    /// A delivery of an event handled before: skipped.
    Repeat(StripeEvent),
}

#[derive(Debug, Error)]
pub enum StripeEventError {
    #[error("only the server takes Stripe's events")]
    ServerOnly,
    #[error("the Stripe-Signature does not match the body: {0}")]
    BadSignature(String),
    #[error("the event could not be read: {0}")]
    Unreadable(String),
    #[error(transparent)]
    Stripe(#[from] StripeError),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// The object an event names, as Stripe has it now.
enum Fetched {
    /// An event nothing here follows.
    Nothing,
    Product(StripeProduct),
    ProductGone(String),
    Price(StripeProduct, StripePrice),
    CheckoutSession(StripeCheckoutSession, Option<StripeSubscription>),
    Subscription(StripeSubscription),
    Charge(StripeCharge),
}

/// Checks a delivery's `Stripe-Signature` header against its body, as
/// sent, with the endpoint's signing secret, `whsec_...`, then handles
/// the event and records it. A signature more than five minutes old is
/// refused.
pub async fn create(
    deployment: &Deployment,
    stripe: &Stripe,
    signing_secret: &SecretString,
    body: &str,
    signature: &str,
) -> Result<StripeEventCreated, StripeEventError> {
    if deployment.role != Role::Server {
        return Err(StripeEventError::ServerOnly);
    }
    let verified = Webhook::construct_event_with_timestamp(
        body,
        signature,
        signing_secret.expose_secret(),
        now().timestamp(),
    );
    match verified {
        // The crate checks the signature before it parses, so a body it
        // cannot type was signed; only the ids are read, as the object
        // is fetched from Stripe.
        Ok(_) | Err(WebhookError::BadParse(_)) => {}
        Err(other) => return Err(StripeEventError::BadSignature(other.to_string())),
    }
    let event: EventBody = serde_json::from_str(body)
        .map_err(|error| StripeEventError::Unreadable(error.to_string()))?;
    let id = event.id.as_str();
    let event_type = event.event_type.as_str();
    if let Some(recorded) = get(deployment, id).await? {
        tracing::info!("stripe event {id} {event_type}: a repeat, skipped");
        return Ok(StripeEventCreated::Repeat(recorded));
    }
    let fetched = fetch(stripe, event_type, event.data.object.id.as_deref()).await?;

    let now = now();
    let mut tx = deployment.begin().await?;
    let inserted = accounts_stripe_events_records::create(
        &mut tx,
        AcceptedStripeEvent(&StripeEvent {
            id: id.to_string(),
            event_type: event_type.to_string(),
            handled_at: now,
        }),
    )
    .await?;
    if inserted > 0 {
        apply_in(&mut tx, fetched, now).await?;
    }
    let recorded = accounts_stripe_events_records::get(&mut tx, id).await?;
    tx.commit().await?;
    let recorded = recorded.ok_or(sqlx::Error::RowNotFound)?;
    if inserted == 0 {
        tracing::info!("stripe event {id} {event_type}: a repeat, skipped");
        return Ok(StripeEventCreated::Repeat(recorded));
    }
    tracing::info!("stripe event {id} {event_type}: handled");
    Ok(StripeEventCreated::New(recorded))
}

async fn fetch(
    stripe: &Stripe,
    event_type: &str,
    object_id: Option<&str>,
) -> Result<Fetched, StripeEventError> {
    let Some(object_id) = object_id else {
        return Ok(Fetched::Nothing);
    };
    let (object, _) = event_type.rsplit_once('.').unwrap_or((event_type, ""));
    match object {
        "product" => match stripe.product_get(object_id).await {
            Ok(product) => Ok(Fetched::Product(product)),
            Err(StripeError::NotFound { .. }) => Ok(Fetched::ProductGone(object_id.to_string())),
            Err(error) => Err(error.into()),
        },
        "price" => {
            let price = stripe.price_get(object_id).await?;
            let product = stripe.product_get(&price.product_id).await?;
            Ok(Fetched::Price(product, price))
        }
        "checkout.session" => {
            let session = stripe.checkout_session_get(object_id).await?;
            let subscription = match &session.subscription_id {
                Some(id) => Some(stripe.subscription_get(id).await?),
                None => None,
            };
            Ok(Fetched::CheckoutSession(session, subscription))
        }
        "subscription_schedule" => {
            match stripe
                .subscription_schedule_subscription_id(object_id)
                .await?
            {
                Some(subscription_id) => Ok(Fetched::Subscription(
                    stripe.subscription_get(&subscription_id).await?,
                )),
                None => Ok(Fetched::Nothing),
            }
        }
        "customer.subscription" => Ok(Fetched::Subscription(
            stripe.subscription_get(object_id).await?,
        )),
        "charge" if event_type == "charge.refunded" => {
            Ok(Fetched::Charge(stripe.charge_get(object_id).await?))
        }
        _ => Ok(Fetched::Nothing),
    }
}

async fn apply_in(
    tx: &mut Transaction,
    fetched: Fetched,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    match fetched {
        Fetched::Nothing => Ok(()),
        Fetched::Product(product) => products::upsert_in(tx, &product, now).await.map(|_| ()),
        Fetched::ProductGone(stripe_id) => {
            products::deactivate_by_stripe_id_in(tx, &stripe_id, now).await
        }
        Fetched::Price(product, price) => {
            let product_id = products::upsert_in(tx, &product, now).await?;
            prices::upsert_in(tx, product_id, &price, now).await
        }
        Fetched::CheckoutSession(session, subscription) => {
            checkouts::apply_session_in(tx, &session, subscription.as_ref(), now).await
        }
        Fetched::Subscription(subscription) => {
            subscriptions::upsert_from_stripe_in(tx, &subscription, now).await
        }
        Fetched::Charge(charge) => match (&charge.payment_intent_id, charge.refunded) {
            (Some(payment_intent_id), true) => {
                purchases::refund_in(tx, Store::Stripe, payment_intent_id, now).await
            }
            _ => Ok(()),
        },
    }
}

/// The event handled with the id, if it was.
pub async fn get(
    deployment: &Deployment,
    id: &str,
) -> Result<Option<StripeEvent>, StripeEventError> {
    if deployment.role != Role::Server {
        return Err(StripeEventError::ServerOnly);
    }
    let mut tx = deployment.begin().await?;
    let event = accounts_stripe_events_records::get(&mut tx, id).await?;
    tx.commit().await?;
    Ok(event)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::with_each_database;

    const SIGNING_SECRET: &str = "whsec_test_secret";

    /// An event as Stripe sends it, with an object no Stripe type has: a
    /// known event type the crate cannot type, an unknown one it keeps.
    fn event_body(id: &str, event_type: &str) -> String {
        event_body_for(id, event_type, "obj_test")
    }

    /// An event naming the object with the id.
    fn event_body_for(id: &str, event_type: &str, object_id: &str) -> String {
        serde_json::json!({
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
        .to_string()
    }

    /// The `Stripe-Signature` Stripe would send with the body, signed so
    /// many seconds ago.
    fn signature_for(body: &str, secret: &str, seconds_ago: i64) -> String {
        Webhook::generate_test_header(body, secret, Some(now().timestamp() - seconds_ago))
    }

    fn secret() -> SecretString {
        SecretString::from(SIGNING_SECRET)
    }

    #[tokio::test]
    async fn stripe_events_receipt_and_catalog_roll_back_and_delivery_can_retry() {
        with_each_database(|database| async move {
            let deployment = Deployment::new(database, Role::Server);
            let product = StripeProduct {
                id: "prod_rollback".to_string(),
                name: "License".to_string(),
                description: None,
                active: true,
            };
            let mut tx = deployment.begin().await.expect("transaction");
            let receipt = StripeEvent {
                id: "evt_rollback".to_string(),
                event_type: "product.created".to_string(),
                handled_at: now(),
            };
            assert_eq!(
                accounts_stripe_events_records::create(&mut tx, AcceptedStripeEvent(&receipt))
                    .await
                    .expect("receipt"),
                1
            );
            products::upsert_in(&mut tx, &product, now())
                .await
                .expect("product");
            drop(tx);
            assert!(
                get(&deployment, &receipt.id)
                    .await
                    .expect("receipt")
                    .is_none()
            );
            let mut tx = deployment.begin().await.expect("transaction");
            assert!(
                crate::data::accounts_products::get_by_stripe_id(&mut tx, &product.id)
                    .await
                    .expect("product")
                    .is_none()
            );
            tx.commit().await.expect("commit");
            let stripe = Stripe::memory();
            stripe.memory_product_put(product);
            let body = event_body_for(&receipt.id, &receipt.event_type, "prod_rollback");
            let signature = signature_for(&body, SIGNING_SECRET, 0);
            assert!(matches!(
                create(&deployment, &stripe, &secret(), &body, &signature)
                    .await
                    .expect("retry"),
                StripeEventCreated::New(_)
            ));
            assert!(matches!(
                create(&deployment, &stripe, &secret(), &body, &signature)
                    .await
                    .expect("repeat"),
                StripeEventCreated::Repeat(_)
            ));
        })
        .await;
    }

    #[tokio::test]
    async fn stripe_events_create_records_an_event_once() {
        with_each_database(|database| async move {
            let deployment = Deployment::new(database, Role::Server);
            let body = event_body("evt_1", "customer.created");
            let signature = signature_for(&body, SIGNING_SECRET, 0);

            let created = create(&deployment, &Stripe::memory(), &secret(), &body, &signature)
                .await
                .expect("recorded");
            let StripeEventCreated::New(event) = created else {
                panic!("the first delivery is new");
            };
            assert_eq!(event.id, "evt_1");
            assert_eq!(event.event_type, "customer.created");
            let again = create(&deployment, &Stripe::memory(), &secret(), &body, &signature)
                .await
                .expect("a repeat");
            assert_eq!(again, StripeEventCreated::Repeat(event.clone()));
            let got = get(&deployment, "evt_1").await.expect("read");
            assert_eq!(got, Some(event));

            let body = event_body("evt_2", "some.future_event");
            let signature = signature_for(&body, SIGNING_SECRET, 0);
            let created = create(&deployment, &Stripe::memory(), &secret(), &body, &signature)
                .await
                .expect("recorded");
            let StripeEventCreated::New(event) = created else {
                panic!("the first delivery is new");
            };
            assert_eq!(event.event_type, "some.future_event");
        })
        .await;
    }

    #[tokio::test]
    async fn stripe_events_create_copies_the_catalog_object_it_names() {
        with_each_database(|database| async move {
            let deployment = Deployment::new(database, Role::Server);
            let stripe = Stripe::memory();
            stripe.memory_product_put(StripeProduct {
                id: "prod_license".to_string(),
                name: "License".to_string(),
                description: Some("Yours for good".to_string()),
                active: true,
            });
            stripe.memory_price_put(StripePrice {
                id: "price_license".to_string(),
                product_id: "prod_license".to_string(),
                lookup_key: Some("license".to_string()),
                amount: Some(4900),
                currency: "usd".to_string(),
                interval: None,
                active: true,
            });
            let deliver = |id: &'static str, event_type: &'static str, object_id: &'static str| {
                let deployment = deployment.clone();
                let stripe = stripe.clone();
                async move {
                    let body = event_body_for(id, event_type, object_id);
                    let signature = signature_for(&body, SIGNING_SECRET, 0);
                    create(&deployment, &stripe, &secret(), &body, &signature)
                        .await
                        .expect("handled")
                }
            };

            deliver("evt_1", "price.created", "price_license").await;
            let for_sale = products::list(&deployment).await.expect("the catalog");
            assert_eq!(for_sale.len(), 1);
            assert_eq!(for_sale[0].name, "License");
            assert_eq!(for_sale[0].prices[0].amount, 4900);

            stripe.memory_product_put(StripeProduct {
                id: "prod_license".to_string(),
                name: "Lifetime license".to_string(),
                description: None,
                active: true,
            });
            deliver("evt_2", "product.updated", "prod_license").await;
            let for_sale = products::list(&deployment).await.expect("the catalog");
            assert_eq!(for_sale[0].name, "Lifetime license");
            assert_eq!(
                for_sale[0].id,
                products::list(&deployment).await.expect("again")[0].id
            );

            if let Stripe::Memory(memory) = &stripe {
                memory.lock().expect("the memory").products.clear();
            }
            deliver("evt_3", "product.deleted", "prod_license").await;
            assert!(
                products::list(&deployment)
                    .await
                    .expect("the catalog")
                    .is_empty()
            );
        })
        .await;
    }

    #[tokio::test]
    async fn stripe_events_create_refuses_a_bad_signature() {
        with_each_database(|database| async move {
            let deployment = Deployment::new(database, Role::Server);
            let body = event_body("evt_1", "customer.created");
            let cases = [
                signature_for(&body, "whsec_another_secret", 0),
                signature_for(&event_body("evt_2", "customer.created"), SIGNING_SECRET, 0),
                signature_for(&body, SIGNING_SECRET, 10 * 60),
                "not a signature".to_string(),
            ];
            for signature in cases {
                let error = create(&deployment, &Stripe::memory(), &secret(), &body, &signature)
                    .await
                    .expect_err("refused");
                assert!(matches!(error, StripeEventError::BadSignature(_)));
            }
            assert_eq!(get(&deployment, "evt_1").await.expect("read"), None);
        })
        .await;
    }

    #[tokio::test]
    async fn stripe_events_create_refuses_a_body_that_is_no_event() {
        with_each_database(|database| async move {
            let deployment = Deployment::new(database, Role::Server);
            let body = r#"{"hello": "world"}"#;
            let signature = signature_for(body, SIGNING_SECRET, 0);
            let error = create(&deployment, &Stripe::memory(), &secret(), body, &signature)
                .await
                .expect_err("unreadable");
            assert!(matches!(error, StripeEventError::Unreadable(_)));
        })
        .await;
    }

    #[tokio::test]
    async fn stripe_events_are_the_servers_alone() {
        with_each_database(|database| async move {
            let deployment = Deployment::new(database, Role::Device);
            let body = event_body("evt_1", "customer.created");
            let signature = signature_for(&body, SIGNING_SECRET, 0);
            let error = create(&deployment, &Stripe::memory(), &secret(), &body, &signature)
                .await
                .expect_err("server only");
            assert!(matches!(error, StripeEventError::ServerOnly));
        })
        .await;
    }
}
