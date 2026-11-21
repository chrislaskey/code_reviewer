//! App Store Server Notifications V2, one row of
//! `accounts_app_store_notifications` per notification handled, as
//! `stripe_events` does for Stripe's. Server only. Apple signs each one,
//! which `AppStore::notification_verify` checks, and may send one more
//! than once, so a repeat is recognized by its `notificationUUID` and
//! skipped; the row is written in the same transaction as what the
//! notification changes. A notification only says something changed:
//! the transaction it names is asked of the App Store as it is now, so
//! notifications out of order never leave an older state. A refund is
//! the purchase's revocation, which `store_transactions` marks through
//! `purchases::refund_in`.

use crate::data::accounts_app_store_notifications;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::accounts::store_transactions::{self, StoreTransactionError};
use crate::deployment::{Deployment, Role, now};
use crate::services::app_store::{AppStore, AppStoreError};

/// A record accepted by the app store notifications workflow.
pub(crate) struct AcceptedAppStoreNotification<'a>(&'a AppStoreNotification);

impl AcceptedAppStoreNotification<'_> {
    /// Borrows the accepted record for persistence.
    pub(crate) fn row(&self) -> &AppStoreNotification {
        self.0
    }
}

pub const TABLE: &str = "accounts_app_store_notifications";

/// A notification handled, by Apple's `notificationUUID`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppStoreNotification {
    pub id: String,
    /// Apple's `notificationType`.
    pub notification_type: String,
    pub handled_at: DateTime<Utc>,
}

/// What `create` did with a delivery.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AppStoreNotificationCreated {
    New(AppStoreNotification),
    Repeat(AppStoreNotification),
}

#[derive(Debug, Error)]
pub enum AppStoreNotificationError {
    #[error("only the server takes the App Store's notifications")]
    ServerOnly,
    #[error(transparent)]
    AppStore(#[from] AppStoreError),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// Checks the notification's signature, then handles and records it. A
/// transaction no account here can take, such as one bought with an
/// offer code that no app has sent yet, is logged and recorded, since
/// asking again would change nothing; the app sends it when it can.
pub async fn create(
    deployment: &Deployment,
    app_store: &AppStore,
    signed_payload: &str,
) -> Result<AppStoreNotificationCreated, AppStoreNotificationError> {
    if deployment.role != Role::Server {
        return Err(AppStoreNotificationError::ServerOnly);
    }
    let notification = app_store.notification_verify(signed_payload)?;
    let id = notification.id.as_str();
    let notification_type = notification.notification_type.as_str();
    if let Some(recorded) = get(deployment, id).await? {
        tracing::info!("app store notification {id} {notification_type}: a repeat, skipped");
        return Ok(AppStoreNotificationCreated::Repeat(recorded));
    }
    let current = match &notification.transaction {
        Some(transaction) => Some(app_store.current(transaction).await?),
        None => None,
    };

    let now = now();
    let mut tx = deployment.begin().await?;
    let inserted = accounts_app_store_notifications::create(
        &mut tx,
        AcceptedAppStoreNotification(&AppStoreNotification {
            id: id.to_string(),
            notification_type: notification_type.to_string(),
            handled_at: now,
        }),
    )
    .await?;
    if inserted > 0
        && let Some(current) = &current
    {
        match store_transactions::apply_in(&mut tx, current, None).await {
            Ok(_) => {}
            Err(StoreTransactionError::Database(error)) => return Err(error.into()),
            Err(refused) => {
                tracing::warn!("app store notification {id} {notification_type}: {refused}")
            }
        }
    }
    let recorded = accounts_app_store_notifications::get(&mut tx, id).await?;
    tx.commit().await?;
    let recorded = recorded.ok_or(sqlx::Error::RowNotFound)?;
    if inserted == 0 {
        return Ok(AppStoreNotificationCreated::Repeat(recorded));
    }
    tracing::info!("app store notification {id} {notification_type}: handled");
    Ok(AppStoreNotificationCreated::New(recorded))
}

/// A notification handled before, by its id.
pub async fn get(
    deployment: &Deployment,
    id: &str,
) -> Result<Option<AppStoreNotification>, sqlx::Error> {
    let mut tx = deployment.begin().await?;
    let recorded = accounts_app_store_notifications::get(&mut tx, id).await?;
    tx.commit().await?;
    Ok(recorded)
}

#[cfg(test)]
mod tests {
    use chrono::Duration;

    use super::*;
    use crate::accounts::store_transactions::tests::catalog;
    use crate::accounts::store_transactions::{StoreProof, StoreRow};
    use crate::accounts::subscriptions::SubscriptionStatus;
    use crate::accounts::{app_store_reconcile, plans, purchases, subscriptions};
    use crate::app::users::{self, User};
    use crate::database::with_each_database;
    use crate::services::app_store::{
        AppStoreEnvironment, AppStoreNotification as Signed, AppStoreRenewal, AppStoreStatus,
        AppStoreSubscription, AppStoreTransaction,
    };

    /// A server with the catalog and the App Store's fake, and Alice.
    async fn set_up(database: crate::database::Database) -> (Deployment, AppStore, User) {
        let app_store = AppStore::memory();
        let server = Deployment::new(database, Role::Server).with_app_store(app_store.clone());
        catalog(&server).await;
        let alice = users::create(&server, "alice@example.com", "Alice")
            .await
            .expect("alice");
        (server, app_store, alice)
    }

    fn transaction(user: &User, product_id: &str, transaction_id: &str) -> AppStoreTransaction {
        AppStoreTransaction {
            transaction_id: transaction_id.to_string(),
            original_transaction_id: transaction_id.to_string(),
            product_id: product_id.to_string(),
            app_account_token: Some(user.id),
            is_subscription: product_id.starts_with("single_user"),
            price: Some(9990),
            currency: Some("USD".to_string()),
            expires_at: product_id
                .starts_with("single_user")
                .then(|| now() + Duration::days(30)),
            revoked_at: None,
            in_free_trial: false,
            environment: AppStoreEnvironment::Sandbox,
        }
    }

    fn state(status: AppStoreStatus, transaction: &AppStoreTransaction) -> AppStoreSubscription {
        AppStoreSubscription {
            status,
            transaction: transaction.clone(),
            renewal: Some(AppStoreRenewal {
                will_renew: true,
                product_id: Some(transaction.product_id.clone()),
                renews_at: transaction.expires_at,
            }),
        }
    }

    /// What the app sends after StoreKit's sheet: the signed transaction.
    async fn bought(
        server: &Deployment,
        app_store: &AppStore,
        user: &User,
        signed: &str,
        transaction: &AppStoreTransaction,
    ) -> Result<StoreRow, StoreTransactionError> {
        app_store.memory_signed_transaction_put(signed, transaction.clone());
        let proof = StoreProof::AppStore {
            signed_transaction: signed.to_string(),
        };
        store_transactions::create(server, user, &proof).await
    }

    /// Apple's notification about the transaction, delivered.
    async fn notified(
        server: &Deployment,
        app_store: &AppStore,
        id: &str,
        notification_type: &str,
        transaction: &AppStoreTransaction,
    ) -> AppStoreNotificationCreated {
        let signed = format!("signed {id}");
        app_store.memory_signed_notification_put(
            &signed,
            Signed {
                id: id.to_string(),
                notification_type: notification_type.to_string(),
                transaction: Some(transaction.clone()),
            },
        );
        create(server, app_store, &signed).await.expect("handled")
    }

    async fn status_of(server: &Deployment, user: &User) -> (SubscriptionStatus, bool) {
        let shown = subscriptions::list(server, user).await.expect("listed");
        (
            shown[0].subscription.status,
            shown[0].subscription.cancel_at_period_end,
        )
    }

    #[tokio::test]
    async fn app_store_a_purchase_then_its_refund() {
        with_each_database(|database| async move {
            let (server, app_store, alice) = set_up(database).await;
            let license = transaction(&alice, "license", "500");
            app_store.memory_transaction_put(license.clone());

            let row = bought(&server, &app_store, &alice, "jws license", &license)
                .await
                .expect("recorded");
            let StoreRow::Purchase(id) = row else {
                panic!("a purchase");
            };
            let shown = purchases::get(&server, &alice, id).await.expect("alice's");
            assert_eq!(
                (shown.purchase.amount, shown.purchase.currency.as_str()),
                (999, "usd")
            );

            let refunded = AppStoreTransaction {
                revoked_at: Some(now()),
                ..license
            };
            app_store.memory_transaction_put(refunded.clone());
            notified(&server, &app_store, "n1", "REFUND", &refunded).await;
            let shown = purchases::get(&server, &alice, id).await.expect("alice's");
            assert!(shown.purchase.refunded_at.is_some());
        })
        .await;
    }

    #[tokio::test]
    async fn app_store_a_subscription_renews_fails_downgrades_and_ends() {
        with_each_database(|database| async move {
            let (server, app_store, alice) = set_up(database).await;
            let monthly = transaction(&alice, "single_user_monthly", "100");
            app_store.memory_subscription_put(state(AppStoreStatus::Active, &monthly));
            bought(&server, &app_store, &alice, "jws monthly", &monthly)
                .await
                .expect("subscribed");
            let plan = plans::current(&server, &alice)
                .await
                .expect("read")
                .expect("a plan");
            assert_eq!(plan.store, crate::accounts::purchases::Store::AppStore);

            let renewed = AppStoreTransaction {
                transaction_id: "101".to_string(),
                expires_at: Some(now() + Duration::days(60)),
                ..monthly.clone()
            };
            app_store.memory_subscription_put(state(AppStoreStatus::Active, &renewed));
            notified(&server, &app_store, "n1", "DID_RENEW", &renewed).await;
            let shown = subscriptions::list(&server, &alice).await.expect("listed");
            assert_eq!(shown.len(), 1, "a renewal is the same subscription");
            assert_eq!(shown[0].subscription.current_period_end, renewed.expires_at);

            app_store.memory_subscription_put(state(AppStoreStatus::BillingRetry, &renewed));
            notified(&server, &app_store, "n2", "DID_FAIL_TO_RENEW", &renewed).await;
            assert_eq!(
                status_of(&server, &alice).await.0,
                SubscriptionStatus::PastDue
            );
            assert_eq!(plans::current(&server, &alice).await.expect("read"), None);

            let mut downgrade = state(AppStoreStatus::Active, &renewed);
            downgrade.renewal = Some(AppStoreRenewal {
                will_renew: true,
                product_id: Some("single_user_yearly".to_string()),
                renews_at: renewed.expires_at,
            });
            app_store.memory_subscription_put(downgrade);
            notified(
                &server,
                &app_store,
                "n3",
                "DID_CHANGE_RENEWAL_PREF",
                &renewed,
            )
            .await;
            let shown = subscriptions::list(&server, &alice).await.expect("listed");
            assert_eq!(
                shown[0]
                    .scheduled_price
                    .as_ref()
                    .and_then(|price| price.lookup_key.as_deref()),
                Some("single_user_yearly")
            );

            let mut cancelled = state(AppStoreStatus::Active, &renewed);
            cancelled.renewal = Some(AppStoreRenewal {
                will_renew: false,
                product_id: None,
                renews_at: None,
            });
            app_store.memory_subscription_put(cancelled);
            notified(
                &server,
                &app_store,
                "n4",
                "DID_CHANGE_RENEWAL_STATUS",
                &renewed,
            )
            .await;
            assert_eq!(
                status_of(&server, &alice).await,
                (SubscriptionStatus::Active, true)
            );

            app_store.memory_subscription_put(state(AppStoreStatus::Expired, &renewed));
            notified(&server, &app_store, "n5", "EXPIRED", &renewed).await;
            assert_eq!(
                status_of(&server, &alice).await.0,
                SubscriptionStatus::Canceled
            );
            assert_eq!(plans::current(&server, &alice).await.expect("read"), None);
        })
        .await;
    }

    #[tokio::test]
    async fn app_store_a_repeated_notification_is_skipped() {
        with_each_database(|database| async move {
            let (server, app_store, alice) = set_up(database).await;
            let monthly = transaction(&alice, "single_user_monthly", "100");
            app_store.memory_subscription_put(state(AppStoreStatus::Active, &monthly));
            let first = notified(&server, &app_store, "n1", "SUBSCRIBED", &monthly).await;
            assert!(matches!(first, AppStoreNotificationCreated::New(_)));
            assert!(
                plans::current(&server, &alice)
                    .await
                    .expect("read")
                    .is_some()
            );

            app_store.memory_subscription_put(state(AppStoreStatus::Expired, &monthly));
            let again = notified(&server, &app_store, "n1", "SUBSCRIBED", &monthly).await;
            assert!(matches!(again, AppStoreNotificationCreated::Repeat(_)));
            assert!(
                plans::current(&server, &alice)
                    .await
                    .expect("read")
                    .is_some(),
                "a repeat changes nothing"
            );

            let error = create(&server, &app_store, "forged")
                .await
                .expect_err("not Apple's");
            assert!(matches!(
                error,
                AppStoreNotificationError::AppStore(AppStoreError::NotVerified(_))
            ));
        })
        .await;
    }

    #[tokio::test]
    async fn app_store_another_users_transaction_is_refused() {
        with_each_database(|database| async move {
            let (server, app_store, alice) = set_up(database).await;
            let bob = users::create(&server, "bob@example.com", "Bob")
                .await
                .expect("bob");
            let alices = transaction(&alice, "license", "500");
            app_store.memory_transaction_put(alices.clone());
            let error = bought(&server, &app_store, &bob, "jws alice's", &alices)
                .await
                .expect_err("alice's");
            assert!(matches!(error, StoreTransactionError::SomeoneElses));

            let offer_code = AppStoreTransaction {
                app_account_token: None,
                ..transaction(&alice, "license", "501")
            };
            app_store.memory_transaction_put(offer_code.clone());
            notified(&server, &app_store, "n1", "ONE_TIME_CHARGE", &offer_code).await;
            assert!(
                purchases::list(&server, &bob)
                    .await
                    .expect("listed")
                    .is_empty(),
                "nobody to give it to yet"
            );
            bought(&server, &app_store, &bob, "jws offer code", &offer_code)
                .await
                .expect("the first who sends it");
            let error = bought(&server, &app_store, &alice, "jws offer code", &offer_code)
                .await
                .expect_err("bob's now");
            assert!(matches!(error, StoreTransactionError::SomeoneElses));
        })
        .await;
    }

    #[tokio::test]
    async fn app_store_reconcile_catches_what_no_notification_said() {
        with_each_database(|database| async move {
            let (server, app_store, alice) = set_up(database).await;
            let monthly = transaction(&alice, "single_user_monthly", "100");
            app_store.memory_subscription_put(state(AppStoreStatus::Active, &monthly));
            bought(&server, &app_store, &alice, "jws monthly", &monthly)
                .await
                .expect("subscribed");
            app_store.memory_subscription_put(state(AppStoreStatus::Revoked, &monthly));
            let reconciled = app_store_reconcile::run(&server, &app_store)
                .await
                .expect("reconciled");
            assert_eq!(reconciled.subscriptions, 1);
            assert_eq!(
                status_of(&server, &alice).await.0,
                SubscriptionStatus::Canceled
            );
            let reconciled = app_store_reconcile::run(&server, &app_store)
                .await
                .expect("reconciled");
            assert_eq!(reconciled.subscriptions, 0, "an ended one is asked no more");
        })
        .await;
    }
}
