//! What a phone's store sold, in one shape for every store, see "iOS
//! first, Android without a rewrite" in `docs/design/purchasing-phones.md`.
//! A store's module checks the store's proof and answers
//! `StoreTransaction`s; `apply_in` writes them as rows like a Stripe
//! checkout's, so `plans` and `purchases` treat them alike. It names no
//! store: who the row is for (M3), the catalog's price by the store's
//! product id, which is the lookup key, one plan per user, and a
//! product bought once only once.

use crate::data::accounts_purchases as accounts_purchases_records;
use crate::data::accounts_subscriptions as accounts_subscriptions_records;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::accounts::prices::{self, Price};
use crate::accounts::products::{Grant, ProductKind};
use crate::accounts::purchases::{self, Purchase, Store};
use crate::accounts::subscriptions::{self, Subscription, SubscriptionStatus};
use crate::accounts::users as accounts_users;
use crate::app::users::User;
use crate::database::Transaction;
use crate::deployment::{Deployment, Role, now};

/// The proof of a purchase the app sends, by store.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "store", rename_all = "snake_case")]
pub enum StoreProof {
    /// StoreKit's signed transaction, a JWS.
    AppStore { signed_transaction: String },
}

/// A store's purchase or subscription as the store has it now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoreTransaction {
    pub store: Store,
    /// The user id the store carried, Apple's `appAccountToken`; none
    /// for one bought without it, such as with an offer code.
    pub user_id: Option<Uuid>,
    /// The store's product id, which is the catalog's lookup key.
    pub lookup_key: String,
    pub details: StoreTransactionDetails,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StoreTransactionDetails {
    OneTime {
        /// The store's id for the payment.
        store_id: String,
        /// In the currency's smallest unit.
        amount: i64,
        currency: String,
        refunded_at: Option<DateTime<Utc>>,
    },
    Subscription {
        /// The store's id that stays across renewals.
        store_id: String,
        /// The store's id of the subscription this one replaced, where a
        /// switch makes a new one.
        replaced_store_id: Option<String>,
        status: SubscriptionStatus,
        current_period_end: Option<DateTime<Utc>>,
        cancel_at_period_end: bool,
        ended_at: Option<DateTime<Utc>>,
        /// The plan it renews into, when that differs.
        scheduled_lookup_key: Option<String>,
        scheduled_at: Option<DateTime<Utc>>,
    },
}

impl StoreTransactionDetails {
    pub fn store_id(&self) -> &str {
        match self {
            StoreTransactionDetails::OneTime { store_id, .. }
            | StoreTransactionDetails::Subscription { store_id, .. } => store_id,
        }
    }
}

/// The row a transaction wrote.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum StoreRow {
    Purchase(Uuid),
    Subscription(Uuid),
}

#[derive(Debug, Error)]
pub enum StoreTransactionError {
    #[error("only the server records the stores' purchases")]
    ServerOnly,
    #[error("Buying in this store is not set up here")]
    StoreNotSetUp,
    #[cfg(feature = "app-store")]
    #[error(transparent)]
    AppStore(#[from] crate::services::app_store::AppStoreError),
    #[error("This was bought while signed in as someone else")]
    SomeoneElses,
    #[error("This purchase is for no account here")]
    NoAccount,
    #[error("Buying needs an account: sign up first")]
    NeedsAccount,
    #[error("That product is not for sale")]
    PriceNotForSale,
    #[error("You have this already")]
    AlreadyOwned,
    #[error("You have a plan already")]
    AlreadySubscribed,
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// Records what the signed-in user bought in a store's sheet, from the
/// proof the app sends: checked with the store, then written as the
/// store has it now, M1 to M3. Answers the purchase or subscription.
pub async fn create(
    deployment: &Deployment,
    user: &User,
    proof: &StoreProof,
) -> Result<StoreRow, StoreTransactionError> {
    if deployment.role != Role::Server {
        return Err(StoreTransactionError::ServerOnly);
    }
    let current = match proof {
        StoreProof::AppStore { signed_transaction } => {
            app_store_current(deployment, signed_transaction).await?
        }
    };
    let mut tx = deployment.begin().await?;
    let row = apply_in(&mut tx, &current, Some(user.id)).await?;
    tx.commit().await?;
    Ok(row)
}

#[cfg(feature = "app-store")]
async fn app_store_current(
    deployment: &Deployment,
    signed_transaction: &str,
) -> Result<StoreTransaction, StoreTransactionError> {
    let app_store = deployment
        .app_store
        .as_ref()
        .ok_or(StoreTransactionError::StoreNotSetUp)?;
    let transaction = app_store.transaction_verify(signed_transaction)?;
    Ok(app_store.current(&transaction).await?)
}

#[cfg(not(feature = "app-store"))]
async fn app_store_current(
    _deployment: &Deployment,
    _signed_transaction: &str,
) -> Result<StoreTransaction, StoreTransactionError> {
    Err(StoreTransactionError::StoreNotSetUp)
}

/// Writes what the store sold, in the transaction: the user's purchase
/// or subscription, made or brought up to date. `sent_by` is the
/// signed-in user who sent the proof, none for the store's own
/// notification. M3: a transaction carrying another user's id is
/// refused; one carrying none goes to the user its row is already
/// for, else to the sender.
pub async fn apply_in(
    tx: &mut Transaction,
    transaction: &StoreTransaction,
    sent_by: Option<Uuid>,
) -> Result<StoreRow, StoreTransactionError> {
    let store_id = transaction.details.store_id();
    let existing = match &transaction.details {
        StoreTransactionDetails::OneTime { .. } => {
            accounts_purchases_records::processing::get_ids_by_store_id(
                &mut *tx,
                transaction.store,
                store_id,
            )
            .await?
        }
        StoreTransactionDetails::Subscription { .. } => {
            accounts_subscriptions_records::processing::get_ids_by_store_id(
                &mut *tx,
                transaction.store,
                store_id,
            )
            .await?
        }
    };
    let user_id = owner(transaction.user_id, existing.map(|(_, user)| user), sent_by)?;
    if existing.is_none() {
        live_full_account_in(tx, user_id).await?;
    }
    let price = price_for_sale_in(tx, &transaction.lookup_key).await?;
    let grant = Grant::from_lookup_key(&transaction.lookup_key)
        .ok_or(StoreTransactionError::PriceNotForSale)?;

    match &transaction.details {
        StoreTransactionDetails::OneTime {
            amount,
            currency,
            refunded_at,
            ..
        } => {
            if grant.kind() != ProductKind::OneTime {
                return Err(StoreTransactionError::PriceNotForSale);
            }
            if existing.is_none()
                && !grant.is_repeatable()
                && purchases::owns_in(tx, user_id, price.product_id).await?
            {
                return Err(StoreTransactionError::AlreadyOwned);
            }
            let now = now();
            let purchase = Purchase {
                id: Uuid::now_v7(),
                user_id,
                product_id: price.product_id,
                price_id: price.id,
                checkout_id: None,
                store: transaction.store,
                store_id: store_id.to_string(),
                amount: *amount,
                currency: currency.to_lowercase(),
                refunded_at: *refunded_at,
                inserted_at: now,
                updated_at: now,
            };
            purchases::insert_if_missing_in(tx, &purchase).await?;
            if let Some(refunded_at) = refunded_at {
                purchases::refund_in(tx, transaction.store, store_id, *refunded_at).await?;
            }
            let (id, _) = accounts_purchases_records::processing::get_ids_by_store_id(
                &mut *tx,
                transaction.store,
                store_id,
            )
            .await?
            .ok_or(sqlx::Error::RowNotFound)?;
            Ok(StoreRow::Purchase(id))
        }
        StoreTransactionDetails::Subscription {
            replaced_store_id,
            status,
            current_period_end,
            cancel_at_period_end,
            ended_at,
            scheduled_lookup_key,
            scheduled_at,
            ..
        } => {
            if grant.kind() != ProductKind::Plan {
                return Err(StoreTransactionError::PriceNotForSale);
            }
            let replaced = match replaced_store_id {
                Some(replaced) => {
                    accounts_subscriptions_records::processing::get_ids_by_store_id(
                        &mut *tx,
                        transaction.store,
                        replaced,
                    )
                    .await?
                }
                None => None,
            };
            if existing.is_none()
                && status.gives_plan()
                && on_another_plan_in(tx, user_id, transaction.store, replaced_store_id).await?
            {
                return Err(StoreTransactionError::AlreadySubscribed);
            }
            let scheduled_price = match scheduled_lookup_key {
                Some(lookup_key) => price_for_sale_in(tx, lookup_key).await.ok(),
                None => None,
            };
            let now = now();
            let subscription = Subscription {
                id: Uuid::now_v7(),
                user_id,
                product_id: price.product_id,
                price_id: price.id,
                store: transaction.store,
                store_id: store_id.to_string(),
                status: *status,
                current_period_end: *current_period_end,
                cancel_at_period_end: *cancel_at_period_end,
                ended_at: *ended_at,
                scheduled_price_id: scheduled_price.as_ref().map(|price| price.id),
                scheduled_at: scheduled_price.and(*scheduled_at),
                inserted_at: now,
                updated_at: now,
            };
            subscriptions::upsert_in(&mut *tx, &subscription).await?;
            if let Some((replaced_id, _)) = replaced {
                end_replaced_in(tx, replaced_id, now).await?;
            }
            let (id, _) = accounts_subscriptions_records::processing::get_ids_by_store_id(
                &mut *tx,
                transaction.store,
                store_id,
            )
            .await?
            .ok_or(sqlx::Error::RowNotFound)?;
            Ok(StoreRow::Subscription(id))
        }
    }
}

/// M3: whose the transaction is.
fn owner(
    carried: Option<Uuid>,
    existing: Option<Uuid>,
    sent_by: Option<Uuid>,
) -> Result<Uuid, StoreTransactionError> {
    let owner = carried
        .or(existing)
        .or(sent_by)
        .ok_or(StoreTransactionError::NoAccount)?;
    let disagrees = |other: Option<Uuid>| other.is_some_and(|other| other != owner);
    if disagrees(existing) || disagrees(sent_by) {
        return Err(StoreTransactionError::SomeoneElses);
    }
    Ok(owner)
}

/// Only a full user buys: signed up, not a trial, not deleted.
async fn live_full_account_in(
    tx: &mut Transaction,
    user_id: Uuid,
) -> Result<(), StoreTransactionError> {
    let account = accounts_users::get_in(tx, user_id)
        .await
        .map_err(|_| StoreTransactionError::NoAccount)?
        .ok_or(StoreTransactionError::NoAccount)?;
    if account.trial_expires_at.is_some() || account.confirmed_at.is_none() {
        return Err(StoreTransactionError::NeedsAccount);
    }
    Ok(())
}

/// The active price with the lookup key, for sale.
async fn price_for_sale_in(
    tx: &mut Transaction,
    lookup_key: &str,
) -> Result<Price, StoreTransactionError> {
    prices::list_in(tx)
        .await?
        .into_iter()
        .find(|price| price.active && price.lookup_key.as_deref() == Some(lookup_key))
        .ok_or(StoreTransactionError::PriceNotForSale)
}

/// Whether the user has a plan from a subscription other than this
/// store's one being replaced.
async fn on_another_plan_in(
    tx: &mut Transaction,
    user_id: Uuid,
    store: Store,
    replaced_store_id: &Option<String>,
) -> Result<bool, sqlx::Error> {
    Ok(subscriptions::list_in(tx, user_id)
        .await?
        .into_iter()
        .any(|(subscription, _)| {
            subscription.status.gives_plan()
                && !(subscription.store == store
                    && replaced_store_id.as_deref() == Some(subscription.store_id.as_str()))
        }))
}

/// The subscription a switch replaced, ended now.
async fn end_replaced_in(
    tx: &mut Transaction,
    id: Uuid,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    let Some((replaced, _)) = accounts_subscriptions_records::processing::get(&mut *tx, id).await?
    else {
        return Ok(());
    };
    if replaced.ended_at.is_some() {
        return Ok(());
    }
    let ended = Subscription {
        status: SubscriptionStatus::Canceled,
        cancel_at_period_end: false,
        ended_at: Some(now),
        scheduled_price_id: None,
        scheduled_at: None,
        updated_at: now,
        ..replaced
    };
    subscriptions::upsert_in(&mut *tx, &ended).await
}

#[cfg(test)]
pub(crate) mod tests {
    use chrono::Duration;

    use super::*;
    use crate::accounts::{plans, products};
    use crate::app::users;
    use crate::database::with_each_database;
    use crate::services::stripe::{Stripe, StripePrice, StripeProduct};

    /// The catalog's placeholders, copied from a Stripe fake.
    pub(crate) async fn catalog(server: &Deployment) {
        let stripe = Stripe::memory();
        for (product, name) in [
            ("prod_single", "Single user subscription"),
            ("prod_license", "License"),
            ("prod_credits", "Credits"),
        ] {
            stripe.memory_product_put(StripeProduct {
                id: product.to_string(),
                name: name.to_string(),
                description: None,
                active: true,
            });
        }
        for (price, product, lookup_key, interval) in [
            (
                "price_monthly",
                "prod_single",
                "single_user_monthly",
                Some("month"),
            ),
            (
                "price_yearly",
                "prod_single",
                "single_user_yearly",
                Some("year"),
            ),
            ("price_license", "prod_license", "license", None),
            ("price_credits", "prod_credits", "credits", None),
        ] {
            stripe.memory_price_put(StripePrice {
                id: price.to_string(),
                product_id: product.to_string(),
                lookup_key: Some(lookup_key.to_string()),
                amount: Some(900),
                currency: "usd".to_string(),
                interval: interval.map(str::to_string),
                active: true,
            });
        }
        products::sync(server, &stripe).await.expect("the catalog");
    }

    async fn full_user(server: &Deployment, email: &str) -> User {
        users::create(server, email, "Someone")
            .await
            .expect("a user")
    }

    fn one_time(user_id: Option<Uuid>, lookup_key: &str, store_id: &str) -> StoreTransaction {
        StoreTransaction {
            store: Store::AppStore,
            user_id,
            lookup_key: lookup_key.to_string(),
            details: StoreTransactionDetails::OneTime {
                store_id: store_id.to_string(),
                amount: 999,
                currency: "USD".to_string(),
                refunded_at: None,
            },
        }
    }

    fn subscription(user_id: Option<Uuid>, lookup_key: &str, store_id: &str) -> StoreTransaction {
        StoreTransaction {
            store: Store::AppStore,
            user_id,
            lookup_key: lookup_key.to_string(),
            details: StoreTransactionDetails::Subscription {
                store_id: store_id.to_string(),
                replaced_store_id: None,
                status: SubscriptionStatus::Active,
                current_period_end: Some(now() + Duration::days(30)),
                cancel_at_period_end: false,
                ended_at: None,
                scheduled_lookup_key: None,
                scheduled_at: None,
            },
        }
    }

    async fn apply(
        server: &Deployment,
        transaction: &StoreTransaction,
        sent_by: Option<Uuid>,
    ) -> Result<StoreRow, StoreTransactionError> {
        let mut tx = server.begin().await.expect("a transaction");
        let applied = apply_in(&mut tx, transaction, sent_by).await;
        tx.commit().await.expect("committed");
        applied
    }

    #[tokio::test]
    async fn store_transactions_license_and_refund_follow_transaction_commit() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            catalog(&server).await;
            let alice = full_user(&server, "alice@example.com").await;
            let license = one_time(Some(alice.id), "license", "license_rollback");
            let mut tx = server.begin().await.expect("transaction");
            apply_in(&mut tx, &license, Some(alice.id))
                .await
                .expect("license");
            drop(tx);
            assert!(
                purchases::list(&server, &alice)
                    .await
                    .expect("purchases")
                    .is_empty()
            );
            let row = apply(&server, &license, Some(alice.id))
                .await
                .expect("retry");
            let StoreRow::Purchase(id) = row else {
                panic!("purchase");
            };
            let purchase = purchases::get(&server, &alice, id).await.expect("purchase");
            let mut tx = server.begin().await.expect("transaction");
            assert!(
                purchases::owns_in(&mut tx, alice.id, purchase.purchase.product_id)
                    .await
                    .expect("owned")
            );
            purchases::refund_in(&mut tx, Store::AppStore, "license_rollback", now())
                .await
                .expect("refund");
            assert!(
                !purchases::owns_in(&mut tx, alice.id, purchase.purchase.product_id)
                    .await
                    .expect("refunded")
            );
            drop(tx);
            let mut tx = server.begin().await.expect("transaction");
            assert!(
                purchases::owns_in(&mut tx, alice.id, purchase.purchase.product_id)
                    .await
                    .expect("refund rolled back")
            );
            purchases::refund_in(&mut tx, Store::AppStore, "license_rollback", now())
                .await
                .expect("refund");
            tx.commit().await.expect("commit");
            assert!(
                purchases::get(&server, &alice, id)
                    .await
                    .expect("history retained")
                    .purchase
                    .refunded_at
                    .is_some()
            );
        })
        .await;
    }

    #[tokio::test]
    async fn store_transactions_apply_records_a_purchase_once() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            catalog(&server).await;
            let alice = full_user(&server, "alice@example.com").await;
            let license = one_time(Some(alice.id), "license", "1000");

            let row = apply(&server, &license, Some(alice.id))
                .await
                .expect("applied");
            let again = apply(&server, &license, None).await.expect("a repeat");
            assert_eq!(row, again, "the same row");
            let bought = purchases::list(&server, &alice).await.expect("listed");
            assert_eq!(bought.len(), 1);
            assert_eq!(bought[0].purchase.store, Store::AppStore);
            assert_eq!(bought[0].purchase.currency, "usd");
            assert_eq!(bought[0].product_name, "License");

            let error = apply(&server, &one_time(Some(alice.id), "license", "1001"), None)
                .await
                .expect_err("bought once only once");
            assert!(matches!(error, StoreTransactionError::AlreadyOwned));
            for store_id in ["2000", "2001"] {
                apply(
                    &server,
                    &one_time(Some(alice.id), "credits", store_id),
                    None,
                )
                .await
                .expect("credits are bought again and again");
            }

            let mut refunded = license.clone();
            if let StoreTransactionDetails::OneTime { refunded_at, .. } = &mut refunded.details {
                *refunded_at = Some(now());
            }
            apply(&server, &refunded, None).await.expect("refunded");
            let bought = purchases::list(&server, &alice).await.expect("listed");
            let license = bought
                .iter()
                .find(|shown| shown.purchase.store_id == "1000")
                .expect("the license");
            assert!(license.purchase.refunded_at.is_some());
        })
        .await;
    }

    #[tokio::test]
    async fn store_transactions_apply_gives_each_to_its_own_user() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            catalog(&server).await;
            let alice = full_user(&server, "alice@example.com").await;
            let bob = full_user(&server, "bob@example.com").await;

            let error = apply(
                &server,
                &one_time(Some(alice.id), "license", "1"),
                Some(bob.id),
            )
            .await
            .expect_err("alice's, sent by bob");
            assert!(matches!(error, StoreTransactionError::SomeoneElses));

            let unmarked = one_time(None, "license", "2");
            apply(&server, &unmarked, Some(bob.id))
                .await
                .expect("given to the first who sends it");
            let error = apply(&server, &unmarked, Some(alice.id))
                .await
                .expect_err("bob's now");
            assert!(matches!(error, StoreTransactionError::SomeoneElses));
            apply(&server, &unmarked, None)
                .await
                .expect("the store's notification finds bob's row");
            assert_eq!(
                purchases::list(&server, &bob).await.expect("listed").len(),
                1
            );

            let error = apply(&server, &one_time(None, "credits", "3"), None)
                .await
                .expect_err("nobody to give it to");
            assert!(matches!(error, StoreTransactionError::NoAccount));
            let error = apply(
                &server,
                &one_time(Some(Uuid::now_v7()), "credits", "4"),
                None,
            )
            .await
            .expect_err("no such account");
            assert!(matches!(error, StoreTransactionError::NoAccount));
            let error = apply(&server, &one_time(Some(alice.id), "unknown", "5"), None)
                .await
                .expect_err("not in the catalog");
            assert!(matches!(error, StoreTransactionError::PriceNotForSale));
            let error = apply(
                &server,
                &one_time(Some(alice.id), "single_user_monthly", "6"),
                None,
            )
            .await
            .expect_err("a plan is not bought once");
            assert!(matches!(error, StoreTransactionError::PriceNotForSale));

            let trial = crate::accounts::trial_users::create(&server, 10)
                .await
                .expect("a trial");
            let error = apply(
                &server,
                &one_time(Some(trial.user.id), "license", "7"),
                None,
            )
            .await
            .expect_err("a trial buys nothing");
            assert!(matches!(error, StoreTransactionError::NeedsAccount));
        })
        .await;
    }

    #[tokio::test]
    async fn store_transactions_apply_keeps_one_plan_and_follows_its_changes() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            catalog(&server).await;
            let alice = full_user(&server, "alice@example.com").await;
            let monthly = subscription(Some(alice.id), "single_user_monthly", "orig_1");

            let row = apply(&server, &monthly, Some(alice.id))
                .await
                .expect("subscribed");
            let plan = plans::current(&server, &alice)
                .await
                .expect("read")
                .expect("a plan");
            assert_eq!(plan.store, Store::AppStore);
            assert_eq!(StoreRow::Subscription(plan.subscription_id), row);

            let error = apply(
                &server,
                &subscription(Some(alice.id), "single_user_yearly", "orig_2"),
                Some(alice.id),
            )
            .await
            .expect_err("one plan per user");
            assert!(matches!(error, StoreTransactionError::AlreadySubscribed));

            let mut downgrade = subscription(Some(alice.id), "single_user_yearly", "orig_1");
            if let StoreTransactionDetails::Subscription {
                scheduled_lookup_key,
                scheduled_at,
                ..
            } = &mut downgrade.details
            {
                *scheduled_lookup_key = Some("single_user_monthly".to_string());
                *scheduled_at = Some(now() + Duration::days(365));
            }
            let switched = apply(&server, &downgrade, None).await.expect("renewed");
            assert_eq!(switched, row, "the same subscription, renewed");
            let shown = subscriptions::list(&server, &alice).await.expect("listed");
            assert_eq!(shown.len(), 1);
            assert_eq!(
                shown[0].price.lookup_key.as_deref(),
                Some("single_user_yearly")
            );
            assert_eq!(
                shown[0]
                    .scheduled_price
                    .as_ref()
                    .and_then(|price| price.lookup_key.as_deref()),
                Some("single_user_monthly")
            );

            let mut replacing = subscription(Some(alice.id), "single_user_yearly", "token_2");
            if let StoreTransactionDetails::Subscription {
                replaced_store_id, ..
            } = &mut replacing.details
            {
                *replaced_store_id = Some("orig_1".to_string());
            }
            let replaced_by = apply(&server, &replacing, Some(alice.id))
                .await
                .expect("a switch that makes a new one");
            let shown = subscriptions::list(&server, &alice).await.expect("listed");
            let old = shown
                .iter()
                .find(|shown| shown.subscription.store_id == "orig_1")
                .expect("the old one");
            assert_eq!(old.subscription.status, SubscriptionStatus::Canceled);
            assert!(old.subscription.ended_at.is_some());
            let plan = plans::current(&server, &alice)
                .await
                .expect("read")
                .expect("a plan");
            assert_eq!(StoreRow::Subscription(plan.subscription_id), replaced_by);

            let mut failed = replacing.clone();
            if let StoreTransactionDetails::Subscription { status, .. } = &mut failed.details {
                *status = SubscriptionStatus::PastDue;
            }
            apply(&server, &failed, None)
                .await
                .expect("a failed renewal");
            assert_eq!(plans::current(&server, &alice).await.expect("read"), None);
        })
        .await;
    }
}
