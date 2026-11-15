//! Device settings: what a person chooses on a device, such as working
//! offline or who is signed in, kept in the device's database as a JSON
//! value per key so a new setting needs no migration. Every setting is
//! declared here, with the value a device starts with, and `upgrade`
//! brings a device up to date with that list when the app starts, so a
//! setting is always there to read. Never synced.

use crate::data::device_settings as device_settings_records;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use thiserror::Error;

use crate::database::Transaction;
use crate::deployment::Deployment;

/// Whether the device works offline, making no request, see `sync`.
pub const WORK_OFFLINE: &str = "work_offline";
/// Who is signed in on this device and the token the device sends the
/// server for them, a `users::SignedIn`, or null when nobody is: written
/// by a sign-in and cleared by a sign-out, see `accounts`.
pub const SIGNED_IN: &str = "signed_in";
/// The community the person works in on this device, an id, or null
/// for none chosen yet, see `current_community`.
pub const CURRENT_COMMUNITY: &str = "current_community_id";

/// The account a local user's records were moved to, a
/// `local_users::Moved`, until the app has updated its list of
/// accounts; null otherwise.
pub const LOCAL_USER_MOVED: &str = "local_user_moved";

/// The signed-in person's plan as the server last answered it, an
/// `accounts::online::PlanKept`, or null until the device has asked.
pub const PLAN: &str = "plan";

/// Every setting, with the value a device starts with. Add a setting
/// here, with its key as a constant above, and read it from the module
/// that owns it.
pub fn defaults() -> Vec<(&'static str, Value)> {
    vec![
        (WORK_OFFLINE, Value::Bool(false)),
        (SIGNED_IN, Value::Null),
        (CURRENT_COMMUNITY, Value::Null),
        (LOCAL_USER_MOVED, Value::Null),
        (PLAN, Value::Null),
    ]
}

#[derive(Debug, Error)]
pub enum DeviceSettingsError {
    #[error("device setting '{0}' was never set up")]
    Missing(String),
    #[error("device setting '{key}' holds something unexpected: {source}")]
    Invalid {
        key: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// Brings the device's settings up to date with `defaults`: adds each
/// one that is missing with its first value, and leaves the rest as the
/// person set them. The place for a release that renames or reshapes a
/// setting to rewrite what an earlier release stored. Returns how many
/// were added.
pub async fn upgrade(deployment: &Deployment) -> Result<usize, DeviceSettingsError> {
    let mut tx = deployment.begin().await?;
    let mut added = 0;
    for (key, value) in defaults() {
        if device_settings_records::get(&mut tx, key).await?.is_none() {
            device_settings_records::upsert(&mut tx, key, &value).await?;
            added += 1;
        }
    }
    tx.commit().await?;
    Ok(added)
}

/// The setting's value, which `upgrade` made sure is there.
pub async fn get<T: DeserializeOwned>(
    tx: &mut Transaction,
    key: &str,
) -> Result<T, DeviceSettingsError> {
    let value = device_settings_records::get(&mut *tx, key)
        .await?
        .ok_or_else(|| DeviceSettingsError::Missing(key.to_string()))?;
    serde_json::from_value(value).map_err(|source| DeviceSettingsError::Invalid {
        key: key.to_string(),
        source,
    })
}

pub async fn set<T: Serialize>(
    tx: &mut Transaction,
    key: &str,
    value: &T,
) -> Result<(), DeviceSettingsError> {
    let value = serde_json::to_value(value).map_err(|source| DeviceSettingsError::Invalid {
        key: key.to_string(),
        source,
    })?;
    Ok(device_settings_records::upsert(&mut *tx, key, &value).await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::with_each_database;
    use crate::deployment::Role;
    use crate::users::SignedIn;

    #[tokio::test]
    async fn device_settings_upgrade_adds_what_is_missing_and_keeps_what_is_set() {
        with_each_database(|database| async move {
            let deployment = Deployment::new(database, Role::Device);
            let mut tx = deployment.begin().await.expect("a transaction");
            let error = get::<bool>(&mut tx, WORK_OFFLINE)
                .await
                .expect_err("not set up");
            assert!(matches!(error, DeviceSettingsError::Missing(key) if key == WORK_OFFLINE));
            tx.commit().await.expect("committed");

            assert_eq!(
                upgrade(&deployment).await.expect("upgraded"),
                defaults().len()
            );
            let mut tx = deployment.begin().await.expect("a transaction");
            assert!(!get::<bool>(&mut tx, WORK_OFFLINE).await.expect("read"));
            assert_eq!(
                get::<Option<SignedIn>>(&mut tx, SIGNED_IN)
                    .await
                    .expect("nobody"),
                None
            );
            set(&mut tx, WORK_OFFLINE, &true).await.expect("set");
            tx.commit().await.expect("committed");

            assert_eq!(upgrade(&deployment).await.expect("upgraded again"), 0);
            let mut tx = deployment.begin().await.expect("a transaction");
            assert!(get::<bool>(&mut tx, WORK_OFFLINE).await.expect("read"));
            let error = get::<String>(&mut tx, WORK_OFFLINE)
                .await
                .expect_err("not a string");
            assert!(matches!(error, DeviceSettingsError::Invalid { .. }));
            tx.commit().await.expect("committed");
        })
        .await;
    }
}
