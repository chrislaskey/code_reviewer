//! The daily job that catches what a missed webhook left behind: the
//! catalog copied again, every checkout still open or processing asked
//! of Stripe, every subscription of a deleted account still live ended,
//! and every subscription not ended copied again. Server
//! only, on a server with Stripe. Asking for every live subscription
//! is fine for a small app; a big one would ask only for what changed.
//! See `docs/design/purchasing.md`.

use crate::data::accounts_checkouts as accounts_checkouts_records;
use crate::data::accounts_subscriptions as accounts_subscriptions_records;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::accounts::products::{self, ProductError};
use crate::accounts::purchases::Store;
use crate::accounts::{checkouts, subscriptions};
use crate::deployment::{Deployment, Role, now};
use crate::services::stripe::{Stripe, StripeError};

/// What the job asked Stripe for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StripeReconciled {
    pub products: usize,
    pub prices: usize,
    pub checkouts: usize,
    pub subscriptions: usize,
}

#[derive(Debug, Error)]
pub enum StripeReconcileError {
    #[error("only the server reconciles with Stripe")]
    ServerOnly,
    #[error(transparent)]
    Product(#[from] ProductError),
    #[error(transparent)]
    Stripe(#[from] StripeError),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

pub async fn run(
    deployment: &Deployment,
    stripe: &Stripe,
) -> Result<StripeReconciled, StripeReconcileError> {
    if deployment.role != Role::Server {
        return Err(StripeReconcileError::ServerOnly);
    }
    let synced = products::sync(deployment, stripe).await?;

    let mut tx = deployment.begin().await?;
    let pending_session_ids =
        accounts_checkouts_records::processing::list_pending_stripe_session_ids(&mut tx).await?;
    let of_deleted_accounts =
        accounts_subscriptions_records::processing::list_live_store_ids_of_deleted_accounts(
            &mut tx,
            Store::Stripe,
        )
        .await?;
    tx.commit().await?;

    for subscription_id in &of_deleted_accounts {
        stripe.subscription_cancel(subscription_id).await?;
    }
    let mut tx = deployment.begin().await?;
    let live_subscription_ids =
        accounts_subscriptions_records::processing::list_live_store_ids(&mut tx, Store::Stripe)
            .await?;
    tx.commit().await?;

    for session_id in &pending_session_ids {
        let session = stripe.checkout_session_get(session_id).await?;
        let subscription = match &session.subscription_id {
            Some(id) => Some(stripe.subscription_get(id).await?),
            None => None,
        };
        let mut tx = deployment.begin().await?;
        checkouts::apply_session_in(&mut tx, &session, subscription.as_ref(), now()).await?;
        tx.commit().await?;
    }
    for subscription_id in &live_subscription_ids {
        let subscription = stripe.subscription_get(subscription_id).await?;
        let mut tx = deployment.begin().await?;
        subscriptions::upsert_from_stripe_in(&mut tx, &subscription, now()).await?;
        tx.commit().await?;
    }
    Ok(StripeReconciled {
        products: synced.products,
        prices: synced.prices,
        checkouts: pending_session_ids.len(),
        subscriptions: live_subscription_ids.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts::plans;
    use crate::app::users;
    use crate::database::with_each_database;
    use crate::services::stripe::{
        StripeCheckoutSession, StripeCheckoutSessionStatus, StripePaymentStatus, StripePrice,
        StripeProduct, StripeSubscription,
    };

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

    #[tokio::test]
    async fn stripe_reconcile_run_catches_what_no_webhook_said() {
        with_each_database(|database| async move {
            let deployment = Deployment::new(database, Role::Server);
            let stripe = Stripe::memory();
            stripe.memory_product_put(StripeProduct {
                id: "prod_single".to_string(),
                name: "Single user subscription".to_string(),
                description: None,
                active: true,
            });
            stripe.memory_price_put(StripePrice {
                id: "price_monthly".to_string(),
                product_id: "prod_single".to_string(),
                lookup_key: Some("single_user_monthly".to_string()),
                amount: Some(900),
                currency: "usd".to_string(),
                interval: Some("month".to_string()),
                active: true,
            });
            let reconciled = run(&deployment, &stripe).await.expect("reconciled");
            assert_eq!((reconciled.products, reconciled.prices), (1, 1));

            let alice = users::create(&deployment, "alice@example.com", "Alice")
                .await
                .expect("a user");
            let monthly = products::list(&deployment).await.expect("the catalog")[0].prices[0].id;
            let created = checkouts::create(
                &deployment,
                &stripe,
                &alice,
                monthly,
                "http://server.test",
                crate::accounts::online::ReturnTo::Web,
            )
            .await
            .expect("a checkout");
            let customer_id = match &stripe {
                Stripe::Memory(memory) => {
                    memory.lock().expect("the memory").checkout_sessions_asked[0]
                        .customer_id
                        .clone()
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
            assert_eq!(
                plans::current(&deployment, &alice).await.expect("read"),
                None
            );
            let reconciled = run(&deployment, &stripe).await.expect("reconciled");
            assert_eq!(reconciled.checkouts, 1);
            assert!(
                plans::current(&deployment, &alice)
                    .await
                    .expect("read")
                    .is_some()
            );

            stripe.memory_subscription_put(subscription("past_due", &customer_id));
            let reconciled = run(&deployment, &stripe).await.expect("reconciled");
            assert_eq!((reconciled.checkouts, reconciled.subscriptions), (0, 1));
            assert_eq!(
                plans::current(&deployment, &alice).await.expect("read"),
                None
            );
        })
        .await;
    }
}
