//! The phone's store, behind one trait so the commands are written once
//! for every store, see "iOS first, Android without a rewrite" in
//! `docs/design/purchasing-phones.md`: the store's prices for some
//! lookup keys, its sheet to buy one for a user, the transactions not
//! finished yet, restoring, finishing one once the server has it, and
//! its page of subscriptions. On iOS it is the App Store through our
//! StoreKit plugin; on Android it answers that it is not available yet;
//! the desktop has none and sells through Stripe.

use serde::Serialize;
use serde_json::Value;
#[cfg(mobile)]
use tauri::AppHandle;
use uuid::Uuid;

/// A store's price for a lookup key, in the buyer's currency.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct StorePrice {
    pub lookup_key: String,
    /// As the store shows it, `$9.99`.
    pub display_price: String,
    /// A subscription's length, `month`.
    pub period: Option<String>,
}

/// A transaction the store holds: the proof the server checks, and the
/// id to finish it by.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoreSheetTransaction {
    pub id: String,
    /// What `POST /api/accounts/store-transactions` takes.
    pub proof: Value,
}

/// What the store's sheet ended in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StoreSheetPurchase {
    Purchased(StoreSheetTransaction),
    Cancelled,
    Pending,
}

/// A phone's store.
pub trait StoreSheet {
    /// Its name as the server's rows have it, `app_store`.
    fn store(&self) -> &'static str;
    async fn prices(&self, lookup_keys: &[String]) -> Result<Vec<StorePrice>, String>;
    async fn buy(&self, lookup_key: &str, user_id: Uuid) -> Result<StoreSheetPurchase, String>;
    async fn unfinished(&self) -> Result<Vec<StoreSheetTransaction>, String>;
    async fn restore(&self) -> Result<Vec<StoreSheetTransaction>, String>;
    async fn finish(&self, transaction: &StoreSheetTransaction) -> Result<(), String>;
    async fn subscriptions_manage_open(&self) -> Result<(), String>;
}

/// The store this phone sells through.
#[cfg(target_os = "ios")]
pub fn of(app: &AppHandle) -> AppStoreSheet<'_> {
    AppStoreSheet { app }
}

#[cfg(target_os = "android")]
pub fn of(_app: &AppHandle) -> NotAvailableYet {
    NotAvailableYet
}

/// The App Store, through StoreKit.
#[cfg(target_os = "ios")]
pub struct AppStoreSheet<'a> {
    app: &'a AppHandle,
}

#[cfg(target_os = "ios")]
impl AppStoreSheet<'_> {
    fn storekit(&self) -> &tauri_plugin_storekit::StoreKit<tauri::Wry> {
        use tauri_plugin_storekit::StoreKitExt;
        self.app.storekit()
    }

    fn transaction_of(
        transaction: tauri_plugin_storekit::StoreKitTransaction,
    ) -> StoreSheetTransaction {
        StoreSheetTransaction {
            id: transaction.transaction_id,
            proof: serde_json::json!({
                "store": "app_store",
                "signed_transaction": transaction.signed_transaction,
            }),
        }
    }
}

#[cfg(target_os = "ios")]
impl StoreSheet for AppStoreSheet<'_> {
    fn store(&self) -> &'static str {
        "app_store"
    }

    async fn prices(&self, lookup_keys: &[String]) -> Result<Vec<StorePrice>, String> {
        let products = self
            .storekit()
            .products(lookup_keys)
            .await
            .map_err(|error| error.to_string())?;
        Ok(products
            .into_iter()
            .map(|product| StorePrice {
                lookup_key: product.id,
                display_price: product.display_price,
                period: product.period,
            })
            .collect())
    }

    async fn buy(&self, lookup_key: &str, user_id: Uuid) -> Result<StoreSheetPurchase, String> {
        use tauri_plugin_storekit::StoreKitPurchase;
        let purchase = self
            .storekit()
            .purchase(lookup_key, &user_id.to_string())
            .await
            .map_err(|error| error.to_string())?;
        Ok(match purchase {
            StoreKitPurchase::Purchased(transaction) => {
                StoreSheetPurchase::Purchased(Self::transaction_of(transaction))
            }
            StoreKitPurchase::Cancelled => StoreSheetPurchase::Cancelled,
            StoreKitPurchase::Pending => StoreSheetPurchase::Pending,
        })
    }

    async fn unfinished(&self) -> Result<Vec<StoreSheetTransaction>, String> {
        let transactions = self
            .storekit()
            .unfinished()
            .await
            .map_err(|error| error.to_string())?;
        Ok(transactions.into_iter().map(Self::transaction_of).collect())
    }

    async fn restore(&self) -> Result<Vec<StoreSheetTransaction>, String> {
        let transactions = self
            .storekit()
            .restore()
            .await
            .map_err(|error| error.to_string())?;
        Ok(transactions.into_iter().map(Self::transaction_of).collect())
    }

    async fn finish(&self, transaction: &StoreSheetTransaction) -> Result<(), String> {
        self.storekit()
            .finish(&transaction.id)
            .await
            .map_err(|error| error.to_string())
    }

    async fn subscriptions_manage_open(&self) -> Result<(), String> {
        self.storekit()
            .manage_subscriptions_open()
            .await
            .map_err(|error| error.to_string())
    }
}

/// Google Play, until its effort builds it.
#[cfg(target_os = "android")]
pub struct NotAvailableYet;

#[cfg(target_os = "android")]
const NOT_AVAILABLE_YET: &str = "On Android, purchases are not available yet";

#[cfg(target_os = "android")]
impl StoreSheet for NotAvailableYet {
    fn store(&self) -> &'static str {
        "play_store"
    }

    async fn prices(&self, _lookup_keys: &[String]) -> Result<Vec<StorePrice>, String> {
        Ok(Vec::new())
    }

    async fn buy(&self, _lookup_key: &str, _user_id: Uuid) -> Result<StoreSheetPurchase, String> {
        Err(NOT_AVAILABLE_YET.to_string())
    }

    async fn unfinished(&self) -> Result<Vec<StoreSheetTransaction>, String> {
        Ok(Vec::new())
    }

    async fn restore(&self) -> Result<Vec<StoreSheetTransaction>, String> {
        Err(NOT_AVAILABLE_YET.to_string())
    }

    async fn finish(&self, _transaction: &StoreSheetTransaction) -> Result<(), String> {
        Ok(())
    }

    async fn subscriptions_manage_open(&self) -> Result<(), String> {
        Err(NOT_AVAILABLE_YET.to_string())
    }
}
