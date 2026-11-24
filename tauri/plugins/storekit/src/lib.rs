//! StoreKit 2 for the iPhone app, called from the app's Rust, see
//! `ios/Sources/StoreKitPlugin.swift` and
//! `docs/design/purchasing-phones.md`. Written for this app in place of
//! `tauri-plugin-iap`, which finishes every transaction itself: here a
//! transaction stays unfinished until `finish` is asked, after the
//! server has recorded it. On any other platform there is no StoreKit,
//! and every call answers `NotAvailable`.

use serde::{Deserialize, Serialize};
use tauri::plugin::{Builder, TauriPlugin};
use tauri::{Manager, Runtime};

#[cfg(target_os = "ios")]
tauri::ios_plugin_binding!(init_plugin_storekit);

/// A product as the App Store sells it here, in the buyer's currency.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoreKitProduct {
    pub id: String,
    pub display_name: String,
    /// The price as the App Store shows it, `$9.99`.
    pub display_price: String,
    /// A subscription's length, `month` or `year`.
    pub period: Option<String>,
}

/// A transaction StoreKit holds, with what Apple signed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoreKitTransaction {
    pub transaction_id: String,
    pub product_id: String,
    /// The JWS the server checks.
    pub signed_transaction: String,
}

/// What the App Store's sheet ended in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StoreKitPurchase {
    Purchased(StoreKitTransaction),
    Cancelled,
    /// Waiting on someone, as with Ask to Buy: recorded when the App
    /// Store's notification comes.
    Pending,
}

#[derive(Debug, thiserror::Error)]
pub enum StoreKitError {
    #[error("The App Store is only in the iPhone app")]
    NotAvailable,
    #[error("The App Store refused: {0}")]
    Refused(String),
}

pub type Result<T> = std::result::Result<T, StoreKitError>;

/// The plugin's handle, managed by the app.
pub struct StoreKit<R: Runtime> {
    #[cfg(target_os = "ios")]
    handle: tauri::plugin::PluginHandle<R>,
    #[cfg(not(target_os = "ios"))]
    _runtime: std::marker::PhantomData<fn() -> R>,
}

/// `app.storekit()`, for the app's commands.
pub trait StoreKitExt<R: Runtime> {
    fn storekit(&self) -> &StoreKit<R>;
}

impl<R: Runtime, T: Manager<R>> StoreKitExt<R> for T {
    fn storekit(&self) -> &StoreKit<R> {
        self.state::<StoreKit<R>>().inner()
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("storekit")
        .setup(|app, api| {
            #[cfg(target_os = "ios")]
            let storekit = StoreKit {
                handle: api.register_ios_plugin(init_plugin_storekit)?,
            };
            #[cfg(not(target_os = "ios"))]
            let storekit = {
                let _ = api;
                StoreKit::<R> {
                    _runtime: std::marker::PhantomData,
                }
            };
            app.manage(storekit);
            Ok(())
        })
        .build()
}

#[derive(Deserialize)]
struct ProductsAnswer {
    products: Vec<StoreKitProduct>,
}

#[derive(Deserialize)]
struct TransactionsAnswer {
    transactions: Vec<StoreKitTransaction>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PurchaseAnswer {
    outcome: String,
    #[serde(flatten)]
    transaction: Option<StoreKitTransaction>,
}

impl<R: Runtime> StoreKit<R> {
    /// The products with the ids, as the App Store sells them here.
    pub async fn products(&self, product_ids: &[String]) -> Result<Vec<StoreKitProduct>> {
        let answer: ProductsAnswer = self
            .call("products", serde_json::json!({ "productIds": product_ids }))
            .await?;
        Ok(answer.products)
    }

    /// The App Store's sheet for the product, for the user's id.
    pub async fn purchase(&self, product_id: &str, user_id: &str) -> Result<StoreKitPurchase> {
        let answer: PurchaseAnswer = self
            .call(
                "purchase",
                serde_json::json!({ "productId": product_id, "appAccountToken": user_id }),
            )
            .await?;
        match (answer.outcome.as_str(), answer.transaction) {
            ("purchased", Some(transaction)) => Ok(StoreKitPurchase::Purchased(transaction)),
            ("cancelled", _) => Ok(StoreKitPurchase::Cancelled),
            ("pending", _) => Ok(StoreKitPurchase::Pending),
            (other, _) => Err(StoreKitError::Refused(format!(
                "an unknown outcome, {other}"
            ))),
        }
    }

    /// The transactions not finished yet.
    pub async fn unfinished(&self) -> Result<Vec<StoreKitTransaction>> {
        let answer: TransactionsAnswer = self.call("unfinished", serde_json::json!({})).await?;
        Ok(answer.transactions)
    }

    /// Restore purchases: what this Apple ID is entitled to.
    pub async fn restore(&self) -> Result<Vec<StoreKitTransaction>> {
        let answer: TransactionsAnswer = self.call("restore", serde_json::json!({})).await?;
        Ok(answer.transactions)
    }

    /// Finishes the transaction, once the server has recorded it.
    pub async fn finish(&self, transaction_id: &str) -> Result<()> {
        let _: serde_json::Value = self
            .call(
                "finish",
                serde_json::json!({ "transactionId": transaction_id }),
            )
            .await?;
        Ok(())
    }

    /// The App Store's page of this Apple ID's subscriptions.
    pub async fn manage_subscriptions_open(&self) -> Result<()> {
        let _: serde_json::Value = self
            .call("manageSubscriptionsOpen", serde_json::json!({}))
            .await?;
        Ok(())
    }

    #[cfg(target_os = "ios")]
    async fn call<Answer: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        payload: serde_json::Value,
    ) -> Result<Answer> {
        self.handle
            .run_mobile_plugin_async(method, payload)
            .await
            .map_err(|error| StoreKitError::Refused(error.to_string()))
    }

    #[cfg(not(target_os = "ios"))]
    async fn call<Answer: serde::de::DeserializeOwned>(
        &self,
        _method: &str,
        _payload: serde_json::Value,
    ) -> Result<Answer> {
        Err(StoreKitError::NotAvailable)
    }
}
