//! The subscriptions commands: the signed-in person's plans, a change
//! to one, and where to manage it: Stripe's Customer Portal on the
//! desktop, the store's own page on a phone. Each asks the server's API
//! as the signed-in person, see `app_core::accounts::online`, and
//! answers its JSON as it came.

pub mod portal;

use serde_json::Value;
use tauri::{AppHandle, State};
use uuid::Uuid;

use app_core::accounts::online;

use crate::accounts::{plans, stripe_pages};
use crate::command_error::CommandError;
use crate::device_accounts::DeviceAccounts;
use crate::sync;

#[tauri::command]
pub async fn accounts_subscriptions_list(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
) -> Result<Value, CommandError> {
    let (deployment, transport) = device.active_with_transport().await?;
    online::subscriptions_list(&deployment, &transport)
        .await
        .map_err(|error| sync::online_error(&app, error))
}

#[tauri::command]
pub async fn accounts_subscriptions_get(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    id: Uuid,
) -> Result<Value, CommandError> {
    let (deployment, transport) = device.active_with_transport().await?;
    online::subscriptions_get(&deployment, &transport, id)
        .await
        .map_err(|error| sync::online_error(&app, error))
}

/// A change to a plan, then the plan kept on the device asked for again.
#[tauri::command]
pub async fn accounts_subscriptions_update(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    id: Uuid,
    change: Value,
) -> Result<Value, CommandError> {
    stripe_pages::refused_on_a_phone()?;
    let (deployment, transport) = device.active_with_transport().await?;
    let changed = online::subscriptions_update(&deployment, &transport, id, &change)
        .await
        .map_err(|error| sync::online_error(&app, error))?;
    plans::refresh(&deployment, &transport).await;
    Ok(changed)
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
