//! The products commands: what is for sale, with the store's own prices
//! on a phone, see `store_transactions`. Each asks the server's API as
//! the signed-in person, see `app_core::accounts::online`, and answers
//! its JSON as it came.

use serde_json::Value;
use tauri::{AppHandle, State};
use uuid::Uuid;

use app_core::accounts::online;

use crate::accounts::store_transactions;
use crate::command_error::CommandError;
use crate::device_accounts::DeviceAccounts;
use crate::sync;

#[tauri::command]
pub async fn accounts_products_list(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
) -> Result<Value, CommandError> {
    let (deployment, transport) = device.active_with_transport().await?;
    let products = online::products_list(&deployment, &transport)
        .await
        .map_err(|error| sync::online_error(&app, error))?;
    Ok(store_transactions::with_store_prices(&app, products).await)
}

#[tauri::command]
pub async fn accounts_products_get(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    id: Uuid,
) -> Result<Value, CommandError> {
    let (deployment, transport) = device.active_with_transport().await?;
    let product = online::products_get(&deployment, &transport, id)
        .await
        .map_err(|error| sync::online_error(&app, error))?;
    Ok(store_transactions::with_store_prices(&app, product).await)
}
