//! Stripe's Customer Portal for a plan's card, on the desktop.

use serde_json::{Value, json};
use tauri::{AppHandle, State};
use uuid::Uuid;

use app_core::accounts::online;

use crate::accounts::store_transactions::NOT_ON_THIS_SERVER;
use crate::accounts::stripe_pages;
use crate::command_error::CommandError;
use crate::device_accounts::DeviceAccounts;
use crate::sync;

/// Stripe's Customer Portal, opened in the browser: answers no `url`.
#[tauri::command]
pub async fn accounts_subscriptions_portal_create(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    id: Uuid,
) -> Result<Value, CommandError> {
    stripe_pages::refused_on_a_phone()?;
    if !device.active_on_default_server().await {
        return Err(NOT_ON_THIS_SERVER.into());
    }
    let (deployment, transport) = device.active_with_transport().await?;
    let portal = online::subscriptions_portal_create(&deployment, &transport, id)
        .await
        .map_err(|error| sync::online_error(&app, error))?;
    stripe_pages::open(&app, portal.get("url"))?;
    Ok(json!({ "url": null }))
}
