//! The App Store, for the server: checks what Apple signed and asks the
//! App Store Server API for the current state, answering it as
//! `StoreTransaction`s, see `docs/design/purchasing-phones.md`. Nothing
//! here writes a row; `store_transactions` does, for every store.
//!
//! Apple signs each transaction and notification with a chain to its
//! root certificates, checked into `certificates/apple`, so a forged one
//! is refused. The environment, production or sandbox, is the data's:
//! a production build's purchases are checked against production, a
//! TestFlight or sandbox tester's against the sandbox. `Memory` takes
//! transactions and notifications as plain values, for tests, as
//! `Stripe::Memory` does.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

use app_store_server_library::api_client::reqwest_transport::reqwest_http_transport::ReqwestHttpTransport;
use app_store_server_library::app_store_server_api_client::AppStoreServerApiClient;
use app_store_server_library::models::app_store_environment::Environment;
use app_store_server_library::models::auto_renew_status::AutoRenewStatus;
use app_store_server_library::models::jws_renewal_info_decoded_payload::JWSRenewalInfoDecodedPayload;
use app_store_server_library::models::jws_transaction_decoded_payload::JWSTransactionDecodedPayload;
use app_store_server_library::models::offer_type::OfferType;
use app_store_server_library::models::product_type::ProductType;
use app_store_server_library::models::status::Status;
use app_store_server_library::signed_data_verifier::SignedDataVerifier;
use chrono::{DateTime, Utc};
use secrecy::{ExposeSecret, SecretString};
use thiserror::Error;
use uuid::Uuid;

use crate::accounts::purchases::Store;
use crate::accounts::store_transactions::{StoreTransaction, StoreTransactionDetails};
use crate::accounts::subscriptions::SubscriptionStatus;

/// Apple's root certificates, public, see `certificates/apple/README.md`.
const ROOT_CERTIFICATES: [&[u8]; 2] = [
    include_bytes!("../../certificates/apple/AppleRootCA-G3.cer"),
    include_bytes!("../../certificates/apple/AppleRootCA-G2.cer"),
];

/// Currencies with no minor unit, whose Apple price in thousandths is
/// divided by a thousand rather than ten.
const ZERO_DECIMAL_CURRENCIES: &[&str] = &[
    "BIF", "CLP", "DJF", "GNF", "ISK", "JPY", "KMF", "KRW", "MGA", "PYG", "RWF", "UGX", "VND",
    "VUV", "XAF", "XOF", "XPF",
];

/// What the server needs of App Store Connect, from the environment,
/// see `axum/src/app_store.rs`. Never committed.
#[derive(Clone)]
pub struct AppStoreSettings {
    pub bundle_id: String,
    /// The app's Apple ID, a number; needed to check production data.
    pub app_apple_id: Option<i64>,
    pub issuer_id: String,
    pub key_id: String,
    /// The In-App Purchase key, the `.p8` file's PEM.
    pub private_key: SecretString,
}

/// Where a transaction was made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppStoreEnvironment {
    Production,
    Sandbox,
}

/// A transaction as Apple signed it, the fields read here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppStoreTransaction {
    pub transaction_id: String,
    /// The first transaction of a subscription, which stays across its
    /// renewals and switches; the transaction's own for a one-time one.
    pub original_transaction_id: String,
    /// The product's id in App Store Connect: the catalog's lookup key.
    pub product_id: String,
    /// The user id the app bought with.
    pub app_account_token: Option<Uuid>,
    pub is_subscription: bool,
    /// In thousandths of the currency's unit, as Apple gives it.
    pub price: Option<i64>,
    pub currency: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
    /// In an introductory offer that costs nothing.
    pub in_free_trial: bool,
    pub environment: AppStoreEnvironment,
}

/// Apple's status of a subscription.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppStoreStatus {
    Active,
    Expired,
    BillingRetry,
    BillingGracePeriod,
    Revoked,
}

/// What a subscription renews into, from its signed renewal info.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppStoreRenewal {
    pub will_renew: bool,
    /// The product it renews into, when the person switched to another.
    pub product_id: Option<String>,
    pub renews_at: Option<DateTime<Utc>>,
}

/// A subscription as Apple has it now: its status, its latest
/// transaction and its renewal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppStoreSubscription {
    pub status: AppStoreStatus,
    pub transaction: AppStoreTransaction,
    pub renewal: Option<AppStoreRenewal>,
}

/// A notification as Apple signed it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppStoreNotification {
    /// Apple's `notificationUUID`.
    pub id: String,
    /// Apple's `notificationType`, `SUBSCRIBED`, `REFUND`, ...
    pub notification_type: String,
    /// The transaction it is about; none for a `TEST`.
    pub transaction: Option<AppStoreTransaction>,
}

#[derive(Debug, Error)]
pub enum AppStoreError {
    #[error("The App Store did not sign this: {0}")]
    NotVerified(String),
    #[error("The App Store has no {0}")]
    NotFound(String),
    #[error("The App Store's answer is missing {0}")]
    Incomplete(&'static str),
    #[error("could not reach the App Store: {0}")]
    Transport(String),
    #[error("the App Store settings are wrong: {0}")]
    Settings(String),
}

/// Where the server's calls to the App Store go.
#[derive(Clone)]
pub enum AppStore {
    Api(Arc<AppStoreApi>),
    Memory(Arc<Mutex<AppStoreMemory>>),
}

/// Which one; never the key.
impl std::fmt::Debug for AppStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppStore::Api(_) => f.write_str("Api"),
            AppStore::Memory(_) => f.write_str("Memory"),
        }
    }
}

/// The verifier and the API client of each environment: the sandbox
/// always, production once the app's Apple ID is set.
pub struct AppStoreApi {
    environments: Vec<ApiEnvironment>,
}

struct ApiEnvironment {
    environment: AppStoreEnvironment,
    verifier: SignedDataVerifier,
    client: AppStoreServerApiClient<ReqwestHttpTransport>,
}

/// What `Memory` holds: signed strings by what they stand for, and the
/// state the API answers.
#[derive(Debug, Default)]
pub struct AppStoreMemory {
    pub signed_transactions: BTreeMap<String, AppStoreTransaction>,
    pub signed_notifications: BTreeMap<String, AppStoreNotification>,
    /// By transaction id.
    pub transactions: BTreeMap<String, AppStoreTransaction>,
    /// By original transaction id.
    pub subscriptions: BTreeMap<String, AppStoreSubscription>,
}

impl AppStore {
    pub fn api(settings: &AppStoreSettings) -> Result<AppStore, AppStoreError> {
        let roots: Vec<Vec<u8>> = ROOT_CERTIFICATES.iter().map(|der| der.to_vec()).collect();
        let mut environments = Vec::new();
        let mut wanted = vec![(AppStoreEnvironment::Sandbox, Environment::Sandbox)];
        if settings.app_apple_id.is_some() {
            wanted.insert(
                0,
                (AppStoreEnvironment::Production, Environment::Production),
            );
        }
        for (environment, apple) in wanted {
            let verifier = SignedDataVerifier::new(
                roots.clone(),
                apple.clone(),
                settings.bundle_id.clone(),
                settings.app_apple_id,
                true,
            )
            .map_err(|error| AppStoreError::Settings(format!("{error:?}")))?;
            let client = AppStoreServerApiClient::new(
                settings.private_key.expose_secret().as_bytes().to_vec(),
                &settings.key_id,
                &settings.issuer_id,
                &settings.bundle_id,
                apple,
                ReqwestHttpTransport::new(),
            )
            .map_err(|error| AppStoreError::Settings(error.to_string()))?;
            environments.push(ApiEnvironment {
                environment,
                verifier,
                client,
            });
        }
        Ok(AppStore::Api(Arc::new(AppStoreApi { environments })))
    }

    pub fn memory() -> AppStore {
        AppStore::Memory(Arc::new(Mutex::new(AppStoreMemory::default())))
    }

    /// Makes `signed` stand for the transaction in `Memory`.
    pub fn memory_signed_transaction_put(&self, signed: &str, transaction: AppStoreTransaction) {
        if let AppStore::Memory(memory) = self {
            lock(memory)
                .signed_transactions
                .insert(signed.to_string(), transaction);
        }
    }

    /// Makes `signed` stand for the notification in `Memory`.
    pub fn memory_signed_notification_put(&self, signed: &str, notification: AppStoreNotification) {
        if let AppStore::Memory(memory) = self {
            lock(memory)
                .signed_notifications
                .insert(signed.to_string(), notification);
        }
    }

    /// The transaction as the API answers it, by its id, in `Memory`.
    pub fn memory_transaction_put(&self, transaction: AppStoreTransaction) {
        if let AppStore::Memory(memory) = self {
            lock(memory)
                .transactions
                .insert(transaction.transaction_id.clone(), transaction);
        }
    }

    /// The subscription as the API answers it, by its original
    /// transaction id, in `Memory`.
    pub fn memory_subscription_put(&self, subscription: AppStoreSubscription) {
        if let AppStore::Memory(memory) = self {
            lock(memory).subscriptions.insert(
                subscription.transaction.original_transaction_id.clone(),
                subscription,
            );
        }
    }

    /// The transaction StoreKit signed, once its signature checks out.
    pub fn transaction_verify(&self, signed: &str) -> Result<AppStoreTransaction, AppStoreError> {
        match self {
            AppStore::Api(api) => {
                let mut refusals = Vec::new();
                for environment in &api.environments {
                    match environment
                        .verifier
                        .verify_and_decode_signed_transaction(signed)
                    {
                        Ok(decoded) => return transaction_of(&decoded, environment.environment),
                        Err(error) => refusals.push(format!("{error:?}")),
                    }
                }
                Err(AppStoreError::NotVerified(refusals.join(", ")))
            }
            AppStore::Memory(memory) => lock(memory)
                .signed_transactions
                .get(signed)
                .cloned()
                .ok_or_else(|| AppStoreError::NotVerified("unknown to the fake".to_string())),
        }
    }

    /// The notification Apple signed, once its signature checks out.
    pub fn notification_verify(
        &self,
        signed_payload: &str,
    ) -> Result<AppStoreNotification, AppStoreError> {
        match self {
            AppStore::Api(api) => {
                let mut refusals = Vec::new();
                for environment in &api.environments {
                    let decoded = match environment
                        .verifier
                        .verify_and_decode_notification(signed_payload)
                    {
                        Ok(decoded) => decoded,
                        Err(error) => {
                            refusals.push(format!("{error:?}"));
                            continue;
                        }
                    };
                    let transaction = match decoded
                        .data
                        .as_ref()
                        .and_then(|data| data.signed_transaction_info.as_deref())
                    {
                        Some(signed) => Some(transaction_of(
                            &environment
                                .verifier
                                .verify_and_decode_signed_transaction(signed)
                                .map_err(|error| {
                                    AppStoreError::NotVerified(format!("{error:?}"))
                                })?,
                            environment.environment,
                        )?),
                        None => None,
                    };
                    let notification_type = decoded
                        .notification_type
                        .as_ref()
                        .and_then(|kind| serde_json::to_value(kind).ok())
                        .and_then(|kind| kind.as_str().map(str::to_string))
                        .unwrap_or_else(|| "UNKNOWN".to_string());
                    return Ok(AppStoreNotification {
                        id: decoded
                            .notification_uuid
                            .ok_or(AppStoreError::Incomplete("notificationUUID"))?,
                        notification_type,
                        transaction,
                    });
                }
                Err(AppStoreError::NotVerified(refusals.join(", ")))
            }
            AppStore::Memory(memory) => lock(memory)
                .signed_notifications
                .get(signed_payload)
                .cloned()
                .ok_or_else(|| AppStoreError::NotVerified("unknown to the fake".to_string())),
        }
    }

    /// What the transaction bought, as the App Store has it now: a
    /// subscription's status and latest renewal, or a one-time
    /// purchase with any refund.
    pub async fn current(
        &self,
        transaction: &AppStoreTransaction,
    ) -> Result<StoreTransaction, AppStoreError> {
        if transaction.is_subscription {
            let subscription = self
                .subscription_get(
                    &transaction.original_transaction_id,
                    Some(transaction.environment),
                )
                .await?;
            Ok(subscription_transaction(&subscription))
        } else {
            let now = self.transaction_get(transaction).await?;
            Ok(one_time_transaction(&now))
        }
    }

    /// The subscription with the original transaction id as the App
    /// Store has it now; with no environment, production then the
    /// sandbox, as Apple advises.
    pub async fn subscription_get(
        &self,
        original_transaction_id: &str,
        environment: Option<AppStoreEnvironment>,
    ) -> Result<AppStoreSubscription, AppStoreError> {
        let api = match self {
            AppStore::Api(api) => api,
            AppStore::Memory(memory) => {
                return lock(memory)
                    .subscriptions
                    .get(original_transaction_id)
                    .cloned()
                    .ok_or_else(|| AppStoreError::NotFound(original_transaction_id.to_string()));
            }
        };
        for asked in api.environments_for(environment) {
            let response = match asked
                .client
                .get_all_subscription_statuses(original_transaction_id, None)
                .await
            {
                Ok(response) => response,
                Err(error) if is_not_found(&format!("{error:?}")) => continue,
                Err(error) => return Err(AppStoreError::Transport(format!("{error:?}"))),
            };
            let latest = response
                .data
                .unwrap_or_default()
                .into_iter()
                .flat_map(|group| group.last_transactions.unwrap_or_default())
                .find(|item| {
                    item.original_transaction_id.as_deref() == Some(original_transaction_id)
                });
            let Some(latest) = latest else { continue };
            let signed = latest
                .signed_transaction_info
                .ok_or(AppStoreError::Incomplete("signedTransactionInfo"))?;
            let decoded = asked
                .verifier
                .verify_and_decode_signed_transaction(&signed)
                .map_err(|error| AppStoreError::NotVerified(format!("{error:?}")))?;
            let renewal = match latest.signed_renewal_info {
                Some(signed) => Some(renewal_of(
                    &asked
                        .verifier
                        .verify_and_decode_renewal_info(&signed)
                        .map_err(|error| AppStoreError::NotVerified(format!("{error:?}")))?,
                )),
                None => None,
            };
            return Ok(AppStoreSubscription {
                status: status_of(latest.status.ok_or(AppStoreError::Incomplete("status"))?)?,
                transaction: transaction_of(&decoded, asked.environment)?,
                renewal,
            });
        }
        Err(AppStoreError::NotFound(original_transaction_id.to_string()))
    }

    /// The one-time transaction as the App Store has it now.
    async fn transaction_get(
        &self,
        transaction: &AppStoreTransaction,
    ) -> Result<AppStoreTransaction, AppStoreError> {
        let api = match self {
            AppStore::Api(api) => api,
            AppStore::Memory(memory) => {
                return lock(memory)
                    .transactions
                    .get(&transaction.transaction_id)
                    .cloned()
                    .ok_or_else(|| AppStoreError::NotFound(transaction.transaction_id.clone()));
            }
        };
        let asked = api
            .environments_for(Some(transaction.environment))
            .next()
            .ok_or_else(|| AppStoreError::NotFound(transaction.transaction_id.clone()))?;
        let response = asked
            .client
            .get_transaction_info(&transaction.transaction_id)
            .await
            .map_err(|error| AppStoreError::Transport(format!("{error:?}")))?;
        let signed = response
            .signed_transaction_info
            .ok_or(AppStoreError::Incomplete("signedTransactionInfo"))?;
        let decoded = asked
            .verifier
            .verify_and_decode_signed_transaction(&signed)
            .map_err(|error| AppStoreError::NotVerified(format!("{error:?}")))?;
        transaction_of(&decoded, asked.environment)
    }
}

impl AppStoreApi {
    /// The environment asked for, or every one this server has.
    fn environments_for(
        &self,
        environment: Option<AppStoreEnvironment>,
    ) -> impl Iterator<Item = &ApiEnvironment> {
        self.environments
            .iter()
            .filter(move |asked| environment.is_none_or(|wanted| asked.environment == wanted))
    }
}

/// Apple's status as ours: billing retry, and a grace period, which the
/// design leaves off in App Store Connect, are no plan; expired and
/// revoked have ended.
pub fn subscription_status(status: AppStoreStatus, in_free_trial: bool) -> SubscriptionStatus {
    match status {
        AppStoreStatus::Active if in_free_trial => SubscriptionStatus::Trialing,
        AppStoreStatus::Active => SubscriptionStatus::Active,
        AppStoreStatus::BillingRetry | AppStoreStatus::BillingGracePeriod => {
            SubscriptionStatus::PastDue
        }
        AppStoreStatus::Expired | AppStoreStatus::Revoked => SubscriptionStatus::Canceled,
    }
}

/// The subscription in the shape every store answers.
pub fn subscription_transaction(subscription: &AppStoreSubscription) -> StoreTransaction {
    let transaction = &subscription.transaction;
    let status = subscription_status(subscription.status, transaction.in_free_trial);
    let ended_at = match subscription.status {
        AppStoreStatus::Revoked => transaction.revoked_at.or(transaction.expires_at),
        AppStoreStatus::Expired => transaction.expires_at,
        _ => None,
    };
    let renewal = subscription.renewal.as_ref();
    let renews_into = renewal
        .and_then(|renewal| renewal.product_id.clone())
        .filter(|product_id| *product_id != transaction.product_id);
    StoreTransaction {
        store: Store::AppStore,
        user_id: transaction.app_account_token,
        lookup_key: transaction.product_id.clone(),
        details: StoreTransactionDetails::Subscription {
            store_id: transaction.original_transaction_id.clone(),
            replaced_store_id: None,
            status,
            current_period_end: transaction.expires_at,
            cancel_at_period_end: ended_at.is_none()
                && renewal.is_some_and(|renewal| !renewal.will_renew),
            ended_at,
            scheduled_at: renews_into.as_ref().and_then(|_| {
                renewal
                    .and_then(|renewal| renewal.renews_at)
                    .or(transaction.expires_at)
            }),
            scheduled_lookup_key: renews_into,
        },
    }
}

/// The one-time purchase in the shape every store answers.
pub fn one_time_transaction(transaction: &AppStoreTransaction) -> StoreTransaction {
    let currency = transaction
        .currency
        .clone()
        .unwrap_or_default()
        .to_uppercase();
    StoreTransaction {
        store: Store::AppStore,
        user_id: transaction.app_account_token,
        lookup_key: transaction.product_id.clone(),
        details: StoreTransactionDetails::OneTime {
            store_id: transaction.transaction_id.clone(),
            amount: minor_units(transaction.price.unwrap_or_default(), &currency),
            currency,
            refunded_at: transaction.revoked_at,
        },
    }
}

/// Apple's price, in thousandths of the unit, in the currency's
/// smallest unit, as the rows keep amounts.
fn minor_units(thousandths: i64, currency: &str) -> i64 {
    if ZERO_DECIMAL_CURRENCIES.contains(&currency) {
        thousandths / 1000
    } else {
        thousandths / 10
    }
}

fn transaction_of(
    decoded: &JWSTransactionDecodedPayload,
    environment: AppStoreEnvironment,
) -> Result<AppStoreTransaction, AppStoreError> {
    let transaction_id = decoded
        .transaction_id
        .clone()
        .ok_or(AppStoreError::Incomplete("transactionId"))?;
    Ok(AppStoreTransaction {
        original_transaction_id: decoded
            .original_transaction_id
            .clone()
            .unwrap_or_else(|| transaction_id.clone()),
        transaction_id,
        product_id: decoded
            .product_id
            .clone()
            .ok_or(AppStoreError::Incomplete("productId"))?,
        app_account_token: decoded.app_account_token,
        is_subscription: matches!(decoded.r#type, Some(ProductType::AutoRenewableSubscription)),
        price: decoded.price,
        currency: decoded.currency.clone(),
        expires_at: decoded.expires_date,
        revoked_at: decoded.revocation_date,
        in_free_trial: matches!(decoded.offer_type, Some(OfferType::IntroductoryOffer))
            && decoded.price == Some(0),
        environment,
    })
}

fn renewal_of(decoded: &JWSRenewalInfoDecodedPayload) -> AppStoreRenewal {
    AppStoreRenewal {
        will_renew: matches!(decoded.auto_renew_status, Some(AutoRenewStatus::On)),
        product_id: decoded.auto_renew_product_id.clone(),
        renews_at: decoded.renewal_date,
    }
}

fn status_of(status: Status) -> Result<AppStoreStatus, AppStoreError> {
    match status {
        Status::Active => Ok(AppStoreStatus::Active),
        Status::Expired => Ok(AppStoreStatus::Expired),
        Status::BillingRetry => Ok(AppStoreStatus::BillingRetry),
        Status::BillingGracePeriod => Ok(AppStoreStatus::BillingGracePeriod),
        Status::Revoked => Ok(AppStoreStatus::Revoked),
        Status::NotSupported(_) => Err(AppStoreError::Incomplete("a status known here")),
    }
}

/// Whether the API said it has no such transaction, Apple's 404.
fn is_not_found(error: &str) -> bool {
    error.contains("404") || error.contains("NotFound")
}

fn lock(memory: &Mutex<AppStoreMemory>) -> std::sync::MutexGuard<'_, AppStoreMemory> {
    memory.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use chrono::Duration;

    use super::*;
    use crate::deployment::now;

    fn monthly(price: i64) -> AppStoreTransaction {
        AppStoreTransaction {
            transaction_id: "2000".to_string(),
            original_transaction_id: "1000".to_string(),
            product_id: "single_user_monthly".to_string(),
            app_account_token: Some(Uuid::now_v7()),
            is_subscription: true,
            price: Some(price),
            currency: Some("USD".to_string()),
            expires_at: Some(now() + Duration::days(30)),
            revoked_at: None,
            in_free_trial: false,
            environment: AppStoreEnvironment::Sandbox,
        }
    }

    #[test]
    fn app_store_status_maps_onto_ours_with_no_grace() {
        use AppStoreStatus::*;
        assert_eq!(
            subscription_status(Active, false),
            SubscriptionStatus::Active
        );
        assert_eq!(
            subscription_status(Active, true),
            SubscriptionStatus::Trialing
        );
        assert_eq!(
            subscription_status(BillingRetry, false),
            SubscriptionStatus::PastDue
        );
        assert_eq!(
            subscription_status(BillingGracePeriod, false),
            SubscriptionStatus::PastDue
        );
        assert_eq!(
            subscription_status(Expired, false),
            SubscriptionStatus::Canceled
        );
        assert_eq!(
            subscription_status(Revoked, false),
            SubscriptionStatus::Canceled
        );
    }

    #[test]
    fn app_store_subscription_transaction_reads_a_downgrade_and_a_cancel() {
        let transaction = monthly(9990);
        let downgrade = AppStoreSubscription {
            status: AppStoreStatus::Active,
            transaction: transaction.clone(),
            renewal: Some(AppStoreRenewal {
                will_renew: true,
                product_id: Some("single_user_yearly".to_string()),
                renews_at: transaction.expires_at,
            }),
        };
        let StoreTransactionDetails::Subscription {
            store_id,
            scheduled_lookup_key,
            scheduled_at,
            cancel_at_period_end,
            ..
        } = subscription_transaction(&downgrade).details
        else {
            panic!("a subscription");
        };
        assert_eq!(store_id, "1000", "by its original transaction");
        assert_eq!(scheduled_lookup_key.as_deref(), Some("single_user_yearly"));
        assert_eq!(scheduled_at, transaction.expires_at);
        assert!(!cancel_at_period_end);

        let cancelled = AppStoreSubscription {
            renewal: Some(AppStoreRenewal {
                will_renew: false,
                product_id: Some("single_user_monthly".to_string()),
                renews_at: None,
            }),
            ..downgrade
        };
        let StoreTransactionDetails::Subscription {
            scheduled_lookup_key,
            cancel_at_period_end,
            ..
        } = subscription_transaction(&cancelled).details
        else {
            panic!("a subscription");
        };
        assert_eq!(scheduled_lookup_key, None);
        assert!(cancel_at_period_end);
    }

    #[test]
    fn app_store_prices_are_kept_in_the_smallest_unit() {
        assert_eq!(minor_units(9990, "USD"), 999);
        assert_eq!(minor_units(1_200_000, "JPY"), 1200);
    }

    #[test]
    fn app_store_api_opens_with_the_roots_checked_in() {
        let settings = AppStoreSettings {
            bundle_id: "com.example.starter-pack".to_string(),
            app_apple_id: Some(1),
            issuer_id: "issuer".to_string(),
            key_id: "KEY".to_string(),
            private_key: SecretString::from("not a key"),
        };
        let store = AppStore::api(&settings).expect("opened");
        let error = store
            .transaction_verify("not.a.jws")
            .expect_err("nothing Apple signed");
        assert!(matches!(error, AppStoreError::NotVerified(_)));
    }
}
