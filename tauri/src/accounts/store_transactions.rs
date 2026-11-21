//! Buying in the phone's store, see `docs/design/purchasing-phones.md`,
//! M1 and M2, through `store_sheet`: the store's sheet, then its proof
//! sent to the server, `app_core::accounts::online`, and only once the
//! server answered is the store's transaction finished. A transaction
//! the server did not get, offline or the app closed, stays unfinished
//! and is sent again when the app starts and after a sign-in. The
//! desktop sells through Stripe and refuses these.

use serde_json::{Value, json};
use tauri::{AppHandle, State};
use uuid::Uuid;

use crate::command_error::CommandError;
use crate::device_accounts::DeviceAccounts;
use crate::sync::SyncServer;

/// The store this app sells through, for the pages to word Buy and
/// Manage by: `app_store`, `play_store`, or `stripe` on the desktop.
#[tauri::command]
pub fn accounts_store_get(app: AppHandle) -> &'static str {
    #[cfg(mobile)]
    {
        use crate::accounts::store_sheet::StoreSheet;
        crate::accounts::store_sheet::of(&app).store()
    }
    #[cfg(desktop)]
    {
        let _ = app;
        "stripe"
    }
}

/// M1: the store's sheet for the price's lookup key, then the proof
/// recorded by the server, then the transaction finished. Answers the
/// server's `{"purchase": ...}` or `{"subscription": ...}`, or
/// `{"cancelled": true}` or `{"pending": true}`.
#[tauri::command]
pub async fn accounts_store_transactions_create(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
    price_id: Uuid,
) -> Result<Value, CommandError> {
    #[cfg(mobile)]
    return on_phone::create(&app, &device, &server, price_id).await;
    #[cfg(desktop)]
    {
        let _ = (app, device, server, price_id);
        Err(ON_THE_DESKTOP.into())
    }
}

/// M2, Restore purchases: what the store says this person has, each
/// recorded by the server, which takes those it lacks. Answers how many
/// were recorded and what was refused.
#[tauri::command]
pub async fn accounts_store_transactions_restore(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
) -> Result<Value, CommandError> {
    #[cfg(mobile)]
    return on_phone::restore(&app, &device, &server).await;
    #[cfg(desktop)]
    {
        let _ = (app, device, server);
        Err(ON_THE_DESKTOP.into())
    }
}

#[cfg(desktop)]
pub const ON_THE_DESKTOP: &str = "On the desktop, plans and products are bought through the web";

/// Sends the store's unfinished transactions, in the background: when
/// the app starts and after a sign-in.
pub fn spawn_send_unfinished(app: AppHandle) {
    #[cfg(mobile)]
    tauri::async_runtime::spawn(async move {
        on_phone::send_unfinished(&app).await;
    });
    #[cfg(desktop)]
    let _ = app;
}

#[cfg(mobile)]
mod on_phone {
    use serde_json::{Value, json};
    use tauri::{AppHandle, Manager};
    use uuid::Uuid;

    use app_core::accounts::online;
    use app_core::deployment::Deployment;

    use crate::accounts::store_sheet::{
        self, StoreSheet, StoreSheetPurchase, StoreSheetTransaction,
    };
    use crate::command_error::CommandError;
    use crate::device_accounts::DeviceAccounts;
    use crate::sync::{SyncServer, online_error};

    pub async fn create(
        app: &AppHandle,
        device: &DeviceAccounts,
        server: &SyncServer,
        price_id: Uuid,
    ) -> Result<Value, CommandError> {
        let deployment = device.active().await?;
        let user_id = device.active_user_id().await.ok_or("Nobody is signed in")?;
        let lookup_key = lookup_key_of(app, &deployment, server, price_id).await?;
        let sheet = store_sheet::of(app);
        match sheet.buy(&lookup_key, user_id).await? {
            StoreSheetPurchase::Purchased(transaction) => {
                let recorded =
                    send_then_finish(app, &sheet, &deployment, server, &transaction).await?;
                crate::accounts::plans::refresh(&deployment, server.transport()).await;
                Ok(recorded)
            }
            StoreSheetPurchase::Cancelled => Ok(json!({ "cancelled": true })),
            StoreSheetPurchase::Pending => Ok(json!({ "pending": true })),
        }
    }

    pub async fn restore(
        app: &AppHandle,
        device: &DeviceAccounts,
        server: &SyncServer,
    ) -> Result<Value, CommandError> {
        let deployment = device.active().await?;
        let sheet = store_sheet::of(app);
        let mut recorded = 0;
        let mut refused = Vec::new();
        for transaction in sheet.restore().await? {
            match send_then_finish(app, &sheet, &deployment, server, &transaction).await {
                Ok(_) => recorded += 1,
                Err(error) => refused.push(error),
            }
        }
        crate::accounts::plans::refresh(&deployment, server.transport()).await;
        Ok(json!({ "recorded": recorded, "refused": refused }))
    }

    pub async fn send_unfinished(app: &AppHandle) {
        let device = app.state::<DeviceAccounts>();
        let server = app.state::<SyncServer>();
        let Ok(deployment) = device.active().await else {
            return;
        };
        let sheet = store_sheet::of(app);
        let unfinished = match sheet.unfinished().await {
            Ok(unfinished) => unfinished,
            Err(error) => {
                log::warn!("could not ask the store for unfinished transactions: {error}");
                return;
            }
        };
        for transaction in &unfinished {
            if let Err(error) =
                send_then_finish(app, &sheet, &deployment, &server, transaction).await
            {
                log::warn!("store transaction {} not sent: {error}", transaction.id);
            }
        }
        if !unfinished.is_empty() {
            crate::accounts::plans::refresh(&deployment, server.transport()).await;
        }
    }

    /// The proof recorded by the server, then the transaction finished;
    /// not finished when the server did not answer.
    async fn send_then_finish(
        app: &AppHandle,
        sheet: &impl StoreSheet,
        deployment: &Deployment,
        server: &SyncServer,
        transaction: &StoreSheetTransaction,
    ) -> Result<Value, CommandError> {
        let recorded =
            online::store_transactions_create(deployment, server.transport(), &transaction.proof)
                .await
                .map_err(|error| online_error(app, error))?;
        sheet.finish(transaction).await?;
        Ok(recorded)
    }

    /// The price's lookup key, which is the store's product id.
    async fn lookup_key_of(
        app: &AppHandle,
        deployment: &Deployment,
        server: &SyncServer,
        price_id: Uuid,
    ) -> Result<String, CommandError> {
        let products = online::products_list(deployment, server.transport())
            .await
            .map_err(|error| online_error(app, error))?;
        let price_id = price_id.to_string();
        products
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|product| product["prices"].as_array().into_iter().flatten())
            .find(|price| price["id"].as_str() == Some(price_id.as_str()))
            .and_then(|price| price["lookup_key"].as_str())
            .map(str::to_string)
            .ok_or_else(|| "That price is not for sale".into())
    }
}

/// The products as the server answers them, with this app's store and,
/// on a phone, each price's in the store's currency, `store_price`, and
/// a plan's length, `store_period`. A price the store does not sell
/// has neither.
pub async fn with_store_prices(app: &AppHandle, products: Value) -> Value {
    let store = accounts_store_get(app.clone());
    let mut products = products;
    #[cfg(mobile)]
    let prices = {
        use crate::accounts::store_sheet::StoreSheet;
        let lookup_keys: Vec<String> = products_iter(&products)
            .flat_map(|product| product["prices"].as_array().cloned().unwrap_or_default())
            .filter_map(|price| price["lookup_key"].as_str().map(str::to_string))
            .collect();
        match crate::accounts::store_sheet::of(app)
            .prices(&lookup_keys)
            .await
        {
            Ok(prices) => prices,
            Err(error) => {
                log::warn!("could not ask the store for its prices: {error}");
                Vec::new()
            }
        }
    };
    #[cfg(desktop)]
    let _ = app;
    for product in products_iter_mut(&mut products) {
        product["store"] = json!(store);
        #[cfg(mobile)]
        for price in product["prices"].as_array_mut().into_iter().flatten() {
            let sold = prices
                .iter()
                .find(|sold| price["lookup_key"].as_str() == Some(sold.lookup_key.as_str()));
            if let Some(sold) = sold {
                price["store_price"] = json!(sold.display_price);
                price["store_period"] = json!(sold.period);
            }
        }
    }
    products
}

/// A list's products, or the one product.
#[cfg(mobile)]
fn products_iter(products: &Value) -> impl Iterator<Item = &Value> {
    match products.as_array() {
        Some(list) => list.iter().collect::<Vec<_>>().into_iter(),
        None => vec![products].into_iter(),
    }
}

fn products_iter_mut(products: &mut Value) -> Vec<&mut Value> {
    if products.is_array() {
        products.as_array_mut().into_iter().flatten().collect()
    } else {
        vec![products]
    }
}
