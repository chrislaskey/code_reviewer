//! The subscriptions commands: the signed-in person's plans, a change
//! to one, and where to manage it: Stripe's Customer Portal on the
//! desktop, the store's own page on a phone. Each asks the server's API
//! as the signed-in person, see `app_core::accounts::online`, and
//! answers its JSON as it came.

use serde_json::{Value, json};
use tauri::{AppHandle, State};
use uuid::Uuid;

use app_core::accounts::online;

use crate::accounts::{plans, stripe_pages};
use crate::command_error::CommandError;
use crate::device_accounts::DeviceAccounts;
use crate::sync::{self, SyncServer};

#[tauri::command]
pub async fn accounts_subscriptions_list(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
) -> Result<Value, CommandError> {
    let deployment = device.active().await?;
    online::subscriptions_list(&deployment, server.transport())
        .await
        .map_err(|error| sync::online_error(&app, error))
}

#[tauri::command]
pub async fn accounts_subscriptions_get(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
    id: Uuid,
) -> Result<Value, CommandError> {
    let deployment = device.active().await?;
    online::subscriptions_get(&deployment, server.transport(), id)
        .await
        .map_err(|error| sync::online_error(&app, error))
}

/// A change to a plan, then the plan kept on the device asked for again.
#[tauri::command]
pub async fn accounts_subscriptions_update(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
    id: Uuid,
    change: Value,
) -> Result<Value, CommandError> {
    stripe_pages::refused_on_a_phone()?;
    let deployment = device.active().await?;
    let changed = online::subscriptions_update(&deployment, server.transport(), id, &change)
        .await
        .map_err(|error| sync::online_error(&app, error))?;
    plans::refresh(&deployment, server.transport()).await;
    Ok(changed)
}

/// Stripe's Customer Portal, opened in the browser: answers no `url`.
#[tauri::command]
pub async fn accounts_subscriptions_portal_create(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
    id: Uuid,
) -> Result<Value, CommandError> {
    stripe_pages::refused_on_a_phone()?;
    let deployment = device.active().await?;
    let portal = online::subscriptions_portal_create(&deployment, server.transport(), id)
        .await
        .map_err(|error| sync::online_error(&app, error))?;
    stripe_pages::open(&app, portal.get("url"))?;
    Ok(json!({ "url": null }))
}

/// The store's own page of subscriptions, where a plan bought there is
/// cancelled or resumed.
#[tauri::command]
pub async fn accounts_subscriptions_manage_open(app: AppHandle) -> Result<(), CommandError> {
    #[cfg(mobile)]
    {
        use crate::accounts::store_sheet::StoreSheet;
        return Ok(crate::accounts::store_sheet::of(&app)
            .subscriptions_manage_open()
            .await?);
    }
    #[cfg(desktop)]
    {
        let _ = app;
        Err(crate::accounts::store_transactions::ON_THE_DESKTOP.into())
    }
}
