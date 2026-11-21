//! The accounts app's purchasing in the desktop app: each command asks
//! the server's API as the signed-in person, see
//! `app_core::accounts::online`, and answers its JSON as it came. Stripe's
//! pages open in the system browser and come back to a page there that
//! says to return to the app, whose pages ask again when it has the
//! focus. A phone buys through its store instead, `store_transactions`,
//! so Stripe's pages are refused there; the products it answers carry
//! the store's prices. See `docs/design/purchasing.md`, step 7, and
//! `docs/design/purchasing-phones.md`.

use serde_json::{Value, json};
use tauri::{AppHandle, State};
use uuid::Uuid;

use app_core::accounts::online;

use crate::device_accounts::DeviceAccounts;
use crate::sync::SyncServer;

#[tauri::command]
pub async fn accounts_products_list(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
) -> Result<Value, String> {
    let deployment = device.active().await.map_err(|error| error.to_string())?;
    let products = online::products_list(&deployment, server.transport())
        .await
        .map_err(|error| crate::sync::online_error(&app, error))?;
    Ok(crate::store_transactions::with_store_prices(&app, products).await)
}

#[tauri::command]
pub async fn accounts_products_get(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
    id: Uuid,
) -> Result<Value, String> {
    let deployment = device.active().await.map_err(|error| error.to_string())?;
    let product = online::products_get(&deployment, server.transport(), id)
        .await
        .map_err(|error| crate::sync::online_error(&app, error))?;
    Ok(crate::store_transactions::with_store_prices(&app, product).await)
}

/// A checkout, its Stripe page opened in the browser: answers the
/// checkout with no `url`, as the app has nothing to go to.
#[tauri::command]
pub async fn accounts_checkouts_create(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
    price_id: Uuid,
) -> Result<Value, String> {
    refused_on_a_phone()?;
    let deployment = device.active().await.map_err(|error| error.to_string())?;
    let created = online::checkouts_create(&deployment, server.transport(), price_id)
        .await
        .map_err(|error| crate::sync::online_error(&app, error))?;
    browser_open(&app, created.get("url"))?;
    Ok(json!({ "checkout": created.get("checkout"), "url": null }))
}

/// A checkout; once it is paid, the plan kept on the device is asked
/// for again.
#[tauri::command]
pub async fn accounts_checkouts_get(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
    id: Uuid,
) -> Result<Value, String> {
    let deployment = device.active().await.map_err(|error| error.to_string())?;
    let checkout = online::checkouts_get(&deployment, server.transport(), id)
        .await
        .map_err(|error| crate::sync::online_error(&app, error))?;
    if checkout.get("status") == Some(&json!("paid")) {
        plan_refresh(&deployment, server.transport()).await;
    }
    Ok(checkout)
}

#[tauri::command]
pub async fn accounts_purchases_list(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
) -> Result<Value, String> {
    let deployment = device.active().await.map_err(|error| error.to_string())?;
    online::purchases_list(&deployment, server.transport())
        .await
        .map_err(|error| crate::sync::online_error(&app, error))
}

#[tauri::command]
pub async fn accounts_purchases_get(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
    id: Uuid,
) -> Result<Value, String> {
    let deployment = device.active().await.map_err(|error| error.to_string())?;
    online::purchases_get(&deployment, server.transport(), id)
        .await
        .map_err(|error| crate::sync::online_error(&app, error))
}

#[tauri::command]
pub async fn accounts_subscriptions_list(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
) -> Result<Value, String> {
    let deployment = device.active().await.map_err(|error| error.to_string())?;
    online::subscriptions_list(&deployment, server.transport())
        .await
        .map_err(|error| crate::sync::online_error(&app, error))
}

#[tauri::command]
pub async fn accounts_subscriptions_get(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
    id: Uuid,
) -> Result<Value, String> {
    let deployment = device.active().await.map_err(|error| error.to_string())?;
    online::subscriptions_get(&deployment, server.transport(), id)
        .await
        .map_err(|error| crate::sync::online_error(&app, error))
}

/// A change to a plan, then the plan kept on the device asked for again.
#[tauri::command]
pub async fn accounts_subscriptions_update(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
    id: Uuid,
    change: Value,
) -> Result<Value, String> {
    refused_on_a_phone()?;
    let deployment = device.active().await.map_err(|error| error.to_string())?;
    let changed = online::subscriptions_update(&deployment, server.transport(), id, &change)
        .await
        .map_err(|error| crate::sync::online_error(&app, error))?;
    plan_refresh(&deployment, server.transport()).await;
    Ok(changed)
}

/// Stripe's Customer Portal, opened in the browser: answers no `url`.
#[tauri::command]
pub async fn accounts_subscriptions_portal_create(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
    id: Uuid,
) -> Result<Value, String> {
    refused_on_a_phone()?;
    let deployment = device.active().await.map_err(|error| error.to_string())?;
    let portal = online::subscriptions_portal_create(&deployment, server.transport(), id)
        .await
        .map_err(|error| crate::sync::online_error(&app, error))?;
    browser_open(&app, portal.get("url"))?;
    Ok(json!({ "url": null }))
}

/// Asks for the plan again, after a change; a failure leaves the plan
/// kept as it was, for the next sync to try.
pub async fn plan_refresh(
    deployment: &app_core::deployment::Deployment,
    server: &app_core::services::web_server::Http,
) {
    if let Err(error) = online::plan_refresh(deployment, server).await {
        log::warn!("could not refresh the plan: {error}");
    }
}

fn browser_open(app: &AppHandle, url: Option<&Value>) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let url = url
        .and_then(Value::as_str)
        .ok_or("The server answered no page to open")?;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|error| error.to_string())
}

/// A phone sells through its store's sheet, see `store_transactions`,
/// never Stripe's page; Android's store is not built yet.
fn refused_on_a_phone() -> Result<(), String> {
    if cfg!(target_os = "android") {
        return Err("On Android, purchases are not available yet".to_string());
    }
    if cfg!(mobile) {
        return Err("On a phone, plans and products are bought in its store".to_string());
    }
    Ok(())
}
