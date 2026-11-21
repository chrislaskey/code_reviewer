//! The accounts app's purchasing from a device: its pages' calls, asked
//! of the server's API with the device's token, their JSON answers passed
//! back as they came, so the app needs none of the server's Stripe code.
//! A device is online only for these, see `docs/design/multi-app.md`.
//! Stripe's pages open in the browser and come back to
//! `/accounts/back-to-app`, see `ReturnTo`.
//!
//! Deleting the account is asked the same way, see `deletions_create`.
//!
//! The plan is the exception, as the device must know it offline:
//! `plan_refresh` keeps the server's answer in the `plan` device
//! setting, and `plan_kept` trusts it until its period ends. See
//! `docs/design/purchasing.md`.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use crate::app::sync::SyncError;
use crate::app::users::UserError;
use crate::deployment::{Deployment, now};
use crate::device_settings::{self, DeviceSettingsError, PLAN};

/// Where Stripe's pages come back to: the web page that asked, or, for
/// the app, a page saying to go back to it, as the browser that paid is
/// not signed in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReturnTo {
    #[default]
    Web,
    App,
}

/// The plan as the device last heard it from the server.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlanKept {
    /// The server's `plan` of the current user, null for none.
    pub plan: Value,
    pub payment_failed_subscription_id: Option<Uuid>,
    pub checked_at: DateTime<Utc>,
}

/// How long a plan kept is fresh enough not to ask again after a sync.
pub const PLAN_FRESH_FOR: Duration = Duration::minutes(15);

#[derive(Debug, Error)]
pub enum OnlineError {
    #[error(transparent)]
    User(#[from] UserError),
    #[error(transparent)]
    Server(#[from] SyncError),
    #[error(transparent)]
    DeviceSettings(#[from] DeviceSettingsError),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// The plan the device may trust now: the one kept, until the period it
/// was paid for ends; none once it has, until the server says again.
pub async fn plan_kept(deployment: &Deployment) -> Result<Option<PlanKept>, OnlineError> {
    let mut tx = deployment.begin().await?;
    let kept: Option<PlanKept> = device_settings::get(&mut tx, PLAN).await?;
    tx.commit().await?;
    Ok(kept.map(|kept| trusted(kept, now())))
}

/// Whether the kept plan is older than `PLAN_FRESH_FOR`, or missing.
pub async fn plan_is_stale(deployment: &Deployment) -> Result<bool, OnlineError> {
    let mut tx = deployment.begin().await?;
    let kept: Option<PlanKept> = device_settings::get(&mut tx, PLAN).await?;
    tx.commit().await?;
    Ok(kept.is_none_or(|kept| now() - kept.checked_at > PLAN_FRESH_FOR))
}

/// The kept plan, with a plan whose period has ended taken away.
fn trusted(mut kept: PlanKept, now: DateTime<Utc>) -> PlanKept {
    let ends: Option<DateTime<Utc>> = kept
        .plan
        .get("current_period_end")
        .and_then(|end| serde_json::from_value(end.clone()).ok());
    if ends.is_some_and(|ends| ends <= now) {
        kept.plan = Value::Null;
    }
    kept
}

#[cfg(feature = "sync-http")]
pub use on_server::*;

#[cfg(feature = "sync-http")]
mod on_server {
    use reqwest::Method;
    use serde_json::{Value, json};
    use uuid::Uuid;

    use super::{OnlineError, PlanKept, ReturnTo};
    use crate::app::sync::Http;
    use crate::app::users;
    use crate::deployment::{Deployment, now};
    use crate::device_settings::{self, PLAN};

    async fn ask(
        deployment: &Deployment,
        server: &Http,
        method: Method,
        path: &str,
        body: Option<&Value>,
    ) -> Result<Value, OnlineError> {
        let (_user, token) = users::signed_in_with_token(deployment).await?;
        Ok(server.json(method, &token, path, body).await?)
    }

    pub async fn products_list(
        deployment: &Deployment,
        server: &Http,
    ) -> Result<Value, OnlineError> {
        ask(
            deployment,
            server,
            Method::GET,
            "/api/accounts/products",
            None,
        )
        .await
    }

    pub async fn products_get(
        deployment: &Deployment,
        server: &Http,
        id: Uuid,
    ) -> Result<Value, OnlineError> {
        let path = format!("/api/accounts/products/{id}");
        ask(deployment, server, Method::GET, &path, None).await
    }

    /// A checkout whose Stripe page comes back to the page for the app.
    pub async fn checkouts_create(
        deployment: &Deployment,
        server: &Http,
        price_id: Uuid,
    ) -> Result<Value, OnlineError> {
        let body = json!({ "price_id": price_id, "return_to": ReturnTo::App });
        ask(
            deployment,
            server,
            Method::POST,
            "/api/accounts/checkouts",
            Some(&body),
        )
        .await
    }

    pub async fn checkouts_get(
        deployment: &Deployment,
        server: &Http,
        id: Uuid,
    ) -> Result<Value, OnlineError> {
        let path = format!("/api/accounts/checkouts/{id}");
        ask(deployment, server, Method::GET, &path, None).await
    }

    pub async fn purchases_list(
        deployment: &Deployment,
        server: &Http,
    ) -> Result<Value, OnlineError> {
        ask(
            deployment,
            server,
            Method::GET,
            "/api/accounts/purchases",
            None,
        )
        .await
    }

    pub async fn purchases_get(
        deployment: &Deployment,
        server: &Http,
        id: Uuid,
    ) -> Result<Value, OnlineError> {
        let path = format!("/api/accounts/purchases/{id}");
        ask(deployment, server, Method::GET, &path, None).await
    }

    pub async fn subscriptions_list(
        deployment: &Deployment,
        server: &Http,
    ) -> Result<Value, OnlineError> {
        ask(
            deployment,
            server,
            Method::GET,
            "/api/accounts/subscriptions",
            None,
        )
        .await
    }

    pub async fn subscriptions_get(
        deployment: &Deployment,
        server: &Http,
        id: Uuid,
    ) -> Result<Value, OnlineError> {
        let path = format!("/api/accounts/subscriptions/{id}");
        ask(deployment, server, Method::GET, &path, None).await
    }

    /// A change to a plan, the server's `SubscriptionChange` as JSON.
    pub async fn subscriptions_update(
        deployment: &Deployment,
        server: &Http,
        id: Uuid,
        change: &Value,
    ) -> Result<Value, OnlineError> {
        let path = format!("/api/accounts/subscriptions/{id}");
        ask(deployment, server, Method::PUT, &path, Some(change)).await
    }

    /// Stripe's Customer Portal, coming back to the page for the app.
    pub async fn subscriptions_portal_create(
        deployment: &Deployment,
        server: &Http,
        id: Uuid,
    ) -> Result<Value, OnlineError> {
        let path = format!("/api/accounts/subscriptions/{id}/portal");
        let body = json!({ "return_to": ReturnTo::App });
        ask(deployment, server, Method::POST, &path, Some(&body)).await
    }

    /// Sends the proof of what a store's sheet sold, `{"store":
    /// "app_store", "signed_transaction": "..."}`, for the server to
    /// check with the store and record: the purchase or subscription.
    pub async fn store_transactions_create(
        deployment: &Deployment,
        server: &Http,
        proof: &Value,
    ) -> Result<Value, OnlineError> {
        let path = "/api/accounts/store-transactions";
        ask(deployment, server, Method::POST, path, Some(proof)).await
    }

    /// Has the server mail the code that confirms deleting the account,
    /// see `accounts::deletions`.
    pub async fn deletions_code_create(
        deployment: &Deployment,
        server: &Http,
    ) -> Result<(), OnlineError> {
        let path = "/api/accounts/deletions/code";
        ask(deployment, server, Method::POST, path, None).await?;
        Ok(())
    }

    /// Deletes the account on the server with the mailed code; the app
    /// then removes it from the device.
    pub async fn deletions_create(
        deployment: &Deployment,
        server: &Http,
        code: &str,
    ) -> Result<(), OnlineError> {
        let body = json!({ "code": code });
        let path = "/api/accounts/deletions";
        ask(deployment, server, Method::POST, path, Some(&body)).await?;
        Ok(())
    }

    /// Asks the server for the signed-in person's plan and keeps it.
    pub async fn plan_refresh(
        deployment: &Deployment,
        server: &Http,
    ) -> Result<PlanKept, OnlineError> {
        let current_user = ask(deployment, server, Method::GET, "/api/current-user", None).await?;
        let kept = PlanKept {
            plan: current_user.get("plan").cloned().unwrap_or(Value::Null),
            payment_failed_subscription_id: current_user
                .get("payment_failed_subscription_id")
                .and_then(|id| serde_json::from_value(id.clone()).ok()),
            checked_at: now(),
        };
        let mut tx = deployment.begin().await?;
        device_settings::set(&mut tx, PLAN, &kept).await?;
        tx.commit().await?;
        Ok(kept)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn kept(current_period_end: Value) -> PlanKept {
        PlanKept {
            plan: json!({ "grant": "single_user_subscription", "current_period_end": current_period_end }),
            payment_failed_subscription_id: None,
            checked_at: now(),
        }
    }

    #[test]
    fn online_plan_kept_is_trusted_until_its_period_ends() {
        let now = now();
        let later = json!(now + Duration::days(3));
        assert_eq!(trusted(kept(later.clone()), now).plan, kept(later).plan);
        let earlier = json!(now - Duration::minutes(1));
        assert_eq!(trusted(kept(earlier), now).plan, Value::Null);
        assert_eq!(
            trusted(kept(Value::Null), now).plan["grant"],
            json!("single_user_subscription")
        );
    }

    #[tokio::test]
    async fn online_plan_kept_is_none_until_asked() {
        crate::database::with_each_database(|database| async move {
            let deployment = Deployment::new(database, crate::deployment::Role::Device);
            device_settings::upgrade(&deployment)
                .await
                .expect("settings");
            assert_eq!(plan_kept(&deployment).await.expect("read"), None);
            assert!(plan_is_stale(&deployment).await.expect("read"));
        })
        .await;
    }
}
