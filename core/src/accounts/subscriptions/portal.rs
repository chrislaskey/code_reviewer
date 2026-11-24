//! Stripe's Customer Portal, where the person changes the card on a
//! plan and sees its invoices.

use uuid::Uuid;

use crate::accounts::checkouts::BACK_TO_APP_PATH;
use crate::accounts::online::ReturnTo;
use crate::accounts::subscriptions::{self, SubscriptionError};
use crate::data::accounts_purchases::Store;
use crate::data::app_users::User;
use crate::deployment::Deployment;
use crate::services::stripe::Stripe;

/// Stripe's Customer Portal for the plan's card and invoices: its page,
/// which comes back to the plan's, `{server_url}/accounts/subscriptions/{id}`,
/// or for the app to `checkouts::BACK_TO_APP_PATH`.
pub async fn create(
    deployment: &Deployment,
    stripe: &Stripe,
    user: &User,
    id: Uuid,
    server_url: &str,
    return_to: ReturnTo,
) -> Result<String, SubscriptionError> {
    let shown = subscriptions::get(deployment, user, id).await?;
    if shown.subscription.store != Store::Stripe {
        return Err(SubscriptionError::OtherStore);
    }
    let customer_id = stripe
        .subscription_get(&shown.subscription.store_id)
        .await?
        .customer_id;
    let return_url = match return_to {
        ReturnTo::Web => format!("{server_url}/accounts/subscriptions/{id}"),
        ReturnTo::App => format!("{server_url}{BACK_TO_APP_PATH}"),
    };
    Ok(stripe
        .billing_portal_session_create(&customer_id, &return_url)
        .await?)
}
