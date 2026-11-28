//! The user's plan: what their one subscription that is active, or in
//! its free days, gives. Server only. A subscription whose payment failed
//! gives nothing, the same as never having bought it; `payment_failed`
//! names it, for the pages to say so. Everything that asks what a user
//! may do asks here, so a phone store later changes nothing that asks.
//! See `docs/design/purchasing.md`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::accounts::products::Grant;
use crate::accounts::subscriptions;
use crate::data::accounts_prices;
use crate::data::accounts_purchases::Store;
use crate::data::accounts_subscriptions::SubscriptionStatus;
use crate::data::app_users::User;
use crate::deployment::{Deployment, Role};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plan {
    pub grant: Grant,
    pub product_id: Uuid,
    pub product_name: String,
    pub price_id: Uuid,
    pub subscription_id: Uuid,
    /// Where it was bought, and so where it is managed.
    pub store: Store,
    pub status: SubscriptionStatus,
    /// The next renewal, or the end when `cancel_at_period_end`.
    pub current_period_end: Option<DateTime<Utc>>,
    pub cancel_at_period_end: bool,
}

#[derive(Debug, Error)]
pub enum PlanError {
    #[error("only the server has plans")]
    ServerOnly,
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// The user's plan, or none.
pub async fn current(deployment: &Deployment, user: &User) -> Result<Option<Plan>, PlanError> {
    if deployment.role != Role::Server {
        return Err(PlanError::ServerOnly);
    }
    // A remote backend sells nothing yet: commerce is not in the sample
    if deployment.has_remote_backend() {
        return Ok(None);
    }
    let mut tx = deployment.begin().await?;
    let subscriptions = subscriptions::list_in(&mut tx, user.id).await?;
    let mut plan = None;
    for (subscription, product_name) in subscriptions {
        if !subscription.status.gives_plan() {
            continue;
        }
        let price = accounts_prices::get(&mut tx, subscription.price_id).await?;
        let Some(grant) = price
            .and_then(|price| price.lookup_key)
            .and_then(|lookup_key| Grant::from_lookup_key(&lookup_key))
        else {
            continue;
        };
        plan = Some(Plan {
            grant,
            product_id: subscription.product_id,
            product_name,
            price_id: subscription.price_id,
            subscription_id: subscription.id,
            store: subscription.store,
            status: subscription.status,
            current_period_end: subscription.current_period_end,
            cancel_at_period_end: subscription.cancel_at_period_end,
        });
        break;
    }
    tx.commit().await?;
    Ok(plan)
}

/// The user's subscription whose payment failed, when they have no plan
/// otherwise: the one to put another card on.
pub async fn payment_failed(
    deployment: &Deployment,
    user: &User,
) -> Result<Option<Uuid>, PlanError> {
    if deployment.role != Role::Server {
        return Err(PlanError::ServerOnly);
    }
    if deployment.has_remote_backend() {
        return Ok(None);
    }
    let mut tx = deployment.begin().await?;
    let subscriptions = subscriptions::list_in(&mut tx, user.id).await?;
    tx.commit().await?;
    if subscriptions
        .iter()
        .any(|(subscription, _)| subscription.status.gives_plan())
    {
        return Ok(None);
    }
    Ok(subscriptions
        .into_iter()
        .find(|(subscription, _)| subscription.status.is_payment_failed())
        .map(|(subscription, _)| subscription.id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts::checkouts::{self, CheckoutError};
    use crate::accounts::{products, stripe_events};
    use crate::app::users;
    use crate::data::accounts_checkouts::CheckoutStatus;
    use crate::database::with_each_database;
    use crate::deployment::now;
    use crate::services::stripe::{
        CheckoutMode, Stripe, StripeCheckoutSession, StripeCheckoutSessionStatus,
        StripePaymentStatus, StripePrice, StripeProduct, StripeSubscription,
    };

    async fn catalog(deployment: &Deployment) -> Stripe {
        let stripe = Stripe::memory();
        stripe.memory_product_put(StripeProduct {
            id: "prod_single".to_string(),
            name: "Single user subscription".to_string(),
            description: None,
            active: true,
        });
        for (id, lookup_key, amount, interval) in [
            ("price_monthly", "single_user_monthly", 900, "month"),
            ("price_yearly", "single_user_yearly", 9000, "year"),
        ] {
            stripe.memory_price_put(StripePrice {
                id: id.to_string(),
                product_id: "prod_single".to_string(),
                lookup_key: Some(lookup_key.to_string()),
                amount: Some(amount),
                currency: "usd".to_string(),
                interval: Some(interval.to_string()),
                active: true,
            });
        }
        products::sync(deployment, &stripe).await.expect("synced");
        stripe
    }

    fn subscription(status: &str, customer_id: &str) -> StripeSubscription {
        StripeSubscription {
            id: "sub_1".to_string(),
            customer_id: customer_id.to_string(),
            item_id: "si_1".to_string(),
            price_id: "price_monthly".to_string(),
            status: status.to_string(),
            current_period_end: Some(now().timestamp() + 30 * 24 * 60 * 60),
            cancel_at_period_end: false,
            ended_at: None,
            schedule_id: None,
            scheduled_price_id: None,
            scheduled_at: None,
        }
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
            "created": now().timestamp(),
            "data": { "object": { "id": object_id, "object": "test_object" } },
            "livemode": false,
            "pending_webhooks": 1,
            "type": event_type,
        })
        .to_string();
        let secret = secrecy::SecretString::from("whsec_test");
        let signature = stripe_webhook::Webhook::generate_test_header(&body, "whsec_test", None);
        stripe_events::create(deployment, stripe, &secret, &body, &signature)
            .await
            .expect("handled");
    }

    /// Alice pays for the monthly plan, as Stripe's return shows it.
    async fn subscribed(deployment: &Deployment, stripe: &Stripe) -> (User, String) {
        let alice = users::create(deployment, "alice@example.com", "Alice")
            .await
            .expect("a user");
        let monthly = products::list(deployment).await.expect("the catalog")[0].prices[0].id;
        let created = checkouts::create(
            deployment,
            stripe,
            &alice,
            monthly,
            "http://server.test",
            crate::accounts::online::ReturnTo::Web,
        )
        .await
        .expect("a checkout");
        let customer_id = match stripe {
            Stripe::Memory(memory) => {
                let memory = memory.lock().expect("the memory");
                let asked = &memory.checkout_sessions_asked[0];
                assert_eq!(asked.mode, CheckoutMode::Subscription);
                asked.customer_id.clone()
            }
            Stripe::Api(_) => unreachable!("tests use memory"),
        };
        stripe.memory_subscription_put(subscription("active", &customer_id));
        stripe.memory_checkout_session_put(StripeCheckoutSession {
            id: created.checkout.stripe_session_id.clone(),
            url: None,
            client_reference_id: Some(created.checkout.id.to_string()),
            status: StripeCheckoutSessionStatus::Complete,
            payment_status: StripePaymentStatus::Paid,
            payment_intent_id: None,
            subscription_id: Some("sub_1".to_string()),
            amount_total: Some(900),
            currency: Some("usd".to_string()),
        });
        let shown = checkouts::get(deployment, stripe, &alice, created.checkout.id)
            .await
            .expect("paid");
        assert_eq!(shown.checkout.status, CheckoutStatus::Paid);
        assert_eq!(shown.purchase_id, None);
        (alice, customer_id)
    }

    #[tokio::test]
    async fn plans_current_is_the_plan_paid_for_and_one_at_a_time() {
        with_each_database(|database| async move {
            let deployment = Deployment::new(database, Role::Server);
            let stripe = catalog(&deployment).await;
            let (alice, _) = subscribed(&deployment, &stripe).await;

            let plan = current(&deployment, &alice)
                .await
                .expect("read")
                .expect("a plan");
            assert_eq!(plan.grant, Grant::SingleUserSubscription);
            assert_eq!(plan.product_name, "Single user subscription");
            assert_eq!(plan.status, SubscriptionStatus::Active);
            assert_eq!(
                payment_failed(&deployment, &alice).await.expect("read"),
                None
            );

            let listed = subscriptions::list(&deployment, &alice)
                .await
                .expect("listed");
            assert_eq!(listed.len(), 1);
            assert_eq!(listed[0].price.amount, 900);
            assert!(listed[0].gives_plan);
            let shown = subscriptions::get(&deployment, &alice, plan.subscription_id)
                .await
                .expect("shown");
            assert_eq!(shown, listed[0]);
            let bob = users::create(&deployment, "bob@example.com", "Bob")
                .await
                .expect("a user");
            let error = subscriptions::get(&deployment, &bob, plan.subscription_id)
                .await
                .expect_err("not his");
            assert!(matches!(
                error,
                subscriptions::SubscriptionError::NotFound { .. }
            ));

            let yearly = products::list(&deployment).await.expect("the catalog")[0].prices[1].id;
            let error = checkouts::create(
                &deployment,
                &stripe,
                &alice,
                yearly,
                "http://server.test",
                crate::accounts::online::ReturnTo::Web,
            )
            .await
            .expect_err("one plan at a time");
            assert!(matches!(error, CheckoutError::AlreadySubscribed));
        })
        .await;
    }

    #[tokio::test]
    async fn plans_current_is_none_once_a_payment_fails_or_the_plan_ends() {
        with_each_database(|database| async move {
            let deployment = Deployment::new(database, Role::Server);
            let stripe = catalog(&deployment).await;
            let (alice, customer_id) = subscribed(&deployment, &stripe).await;
            let subscription_id = current(&deployment, &alice)
                .await
                .expect("read")
                .expect("a plan")
                .subscription_id;

            stripe.memory_subscription_put(subscription("past_due", &customer_id));
            deliver(
                &deployment,
                &stripe,
                "evt_1",
                "customer.subscription.updated",
                "sub_1",
            )
            .await;
            assert_eq!(current(&deployment, &alice).await.expect("read"), None);
            assert_eq!(
                payment_failed(&deployment, &alice).await.expect("read"),
                Some(subscription_id)
            );

            stripe.memory_subscription_put(subscription("active", &customer_id));
            deliver(
                &deployment,
                &stripe,
                "evt_2",
                "customer.subscription.updated",
                "sub_1",
            )
            .await;
            assert!(current(&deployment, &alice).await.expect("read").is_some());

            let mut canceled = subscription("canceled", &customer_id);
            canceled.ended_at = Some(now().timestamp());
            stripe.memory_subscription_put(canceled);
            deliver(
                &deployment,
                &stripe,
                "evt_3",
                "customer.subscription.deleted",
                "sub_1",
            )
            .await;
            assert_eq!(current(&deployment, &alice).await.expect("read"), None);
            assert_eq!(
                payment_failed(&deployment, &alice).await.expect("read"),
                None
            );
            let listed = subscriptions::list(&deployment, &alice)
                .await
                .expect("listed");
            assert_eq!(listed.len(), 1, "the same subscription, updated in place");
            assert!(listed[0].subscription.ended_at.is_some());
        })
        .await;
    }

    #[tokio::test]
    async fn subscriptions_update_switches_now_or_at_the_period_end() {
        use crate::accounts::subscriptions::{SubscriptionChange, SubscriptionError};
        with_each_database(|database| async move {
            let deployment = Deployment::new(database, Role::Server);
            let stripe = catalog(&deployment).await;
            let (alice, _) = subscribed(&deployment, &stripe).await;
            let id = current(&deployment, &alice)
                .await
                .expect("read")
                .expect("a plan")
                .subscription_id;
            let prices = products::list(&deployment).await.expect("the catalog")[0]
                .prices
                .clone();
            let (monthly, yearly) = (prices[0].id, prices[1].id);

            let error = subscriptions::update(
                &deployment,
                &stripe,
                &alice,
                id,
                SubscriptionChange::Switch { price_id: monthly },
            )
            .await
            .expect_err("the same price");
            assert!(matches!(error, SubscriptionError::SamePrice));

            let shown = subscriptions::update(
                &deployment,
                &stripe,
                &alice,
                id,
                SubscriptionChange::Switch { price_id: yearly },
            )
            .await
            .expect("monthly to yearly is at once");
            assert_eq!(shown.price.id, yearly);
            assert_eq!(shown.scheduled_price, None);

            let shown = subscriptions::update(
                &deployment,
                &stripe,
                &alice,
                id,
                SubscriptionChange::Switch { price_id: monthly },
            )
            .await
            .expect("yearly to monthly waits");
            assert_eq!(shown.price.id, yearly);
            assert_eq!(
                shown.scheduled_price.as_ref().map(|price| price.id),
                Some(monthly)
            );
            assert_eq!(
                shown.subscription.scheduled_at,
                shown.subscription.current_period_end
            );

            let shown = subscriptions::update(
                &deployment,
                &stripe,
                &alice,
                id,
                SubscriptionChange::SwitchUndo,
            )
            .await
            .expect("undone");
            assert_eq!(shown.price.id, yearly);
            assert_eq!(shown.scheduled_price, None);
        })
        .await;
    }

    #[tokio::test]
    async fn subscriptions_update_cancels_and_resumes_and_opens_the_portal() {
        use crate::accounts::subscriptions::{SubscriptionChange, SubscriptionError};
        with_each_database(|database| async move {
            let deployment = Deployment::new(database, Role::Server);
            let stripe = catalog(&deployment).await;
            let (alice, customer_id) = subscribed(&deployment, &stripe).await;
            let id = current(&deployment, &alice)
                .await
                .expect("read")
                .expect("a plan")
                .subscription_id;
            let yearly = products::list(&deployment).await.expect("the catalog")[0].prices[1].id;

            let shown =
                subscriptions::update(&deployment, &stripe, &alice, id, SubscriptionChange::Cancel)
                    .await
                    .expect("canceled");
            assert!(shown.subscription.cancel_at_period_end);
            assert!(
                current(&deployment, &alice)
                    .await
                    .expect("read")
                    .expect("still a plan")
                    .cancel_at_period_end
            );
            let error = subscriptions::update(
                &deployment,
                &stripe,
                &alice,
                id,
                SubscriptionChange::Switch { price_id: yearly },
            )
            .await
            .expect_err("resume first");
            assert!(matches!(error, SubscriptionError::Canceling));
            let shown =
                subscriptions::update(&deployment, &stripe, &alice, id, SubscriptionChange::Resume)
                    .await
                    .expect("resumed");
            assert!(!shown.subscription.cancel_at_period_end);

            let url = subscriptions::portal::create(
                &deployment,
                &stripe,
                &alice,
                id,
                "http://server.test",
                crate::accounts::online::ReturnTo::Web,
            )
            .await
            .expect("the portal");
            assert!(url.contains(&customer_id));
            assert!(url.contains(&format!("http://server.test/accounts/subscriptions/{id}")));

            stripe.memory_subscription_put(subscription("past_due", &customer_id));
            deliver(
                &deployment,
                &stripe,
                "evt_1",
                "customer.subscription.updated",
                "sub_1",
            )
            .await;
            let error = subscriptions::update(
                &deployment,
                &stripe,
                &alice,
                id,
                SubscriptionChange::Switch { price_id: yearly },
            )
            .await
            .expect_err("the card first");
            assert!(matches!(error, SubscriptionError::PaymentFailed));
            subscriptions::portal::create(
                &deployment,
                &stripe,
                &alice,
                id,
                "http://server.test",
                crate::accounts::online::ReturnTo::Web,
            )
            .await
            .expect("the portal still opens, for the card");
        })
        .await;
    }
}
