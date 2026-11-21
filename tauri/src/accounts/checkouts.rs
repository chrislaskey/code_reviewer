//! The checkouts commands: buying a price on Stripe's page, opened in
//! the system browser, which comes back to a page there that says to
//! return to the app, whose pages ask again when it has the focus. A
//! phone buys through its store instead, `store_transactions`. Each
//! asks the server's API as the signed-in person, see
//! `app_core::accounts::online`, and answers its JSON as it came. See
//! `docs/design/purchasing.md`, step 7.

use serde_json::{Value, json};
use tauri::{AppHandle, State};
use uuid::Uuid;

use app_core::accounts::online;

use crate::accounts::{plans, stripe_pages};
use crate::command_error::CommandError;
use crate::device_accounts::DeviceAccounts;
use crate::sync::{self, SyncServer};

/// A checkout, its Stripe page opened in the browser: answers the
/// checkout with no `url`, as the app has nothing to go to.
#[tauri::command]
pub async fn accounts_checkouts_create(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
    price_id: Uuid,
) -> Result<Value, CommandError> {
    stripe_pages::refused_on_a_phone()?;
    let deployment = device.active().await?;
    let created = online::checkouts_create(&deployment, server.transport(), price_id)
        .await
        .map_err(|error| sync::online_error(&app, error))?;
    stripe_pages::open(&app, created.get("url"))?;
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
) -> Result<Value, CommandError> {
    let deployment = device.active().await?;
    let checkout = online::checkouts_get(&deployment, server.transport(), id)
        .await
        .map_err(|error| sync::online_error(&app, error))?;
    if checkout.get("status") == Some(&json!("paid")) {
        plans::refresh(&deployment, server.transport()).await;
    }
    Ok(checkout)
}
