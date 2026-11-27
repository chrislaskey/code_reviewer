//! Device settings: what a person chooses on a device, such as working
//! offline or who is signed in, kept in the device's database as a JSON
//! value per key so a new setting needs no migration. Every setting is
//! declared here, with the value a device starts with, and
//! `crate::device_settings::upgrade`
//! brings a device up to date with that list when the app starts, so a
//! setting is always there to read. Never synced. The reads and
//! writes of `device_settings`.

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use thiserror::Error;

use crate::database::Transaction;

#[cfg(feature = "postgres")]
mod postgres;
mod sqlite;

macro_rules! query {
    ($tx:expr, $function:ident($($argument:expr),* $(,)?)) => {
        match $tx {
            Transaction::Sqlite(tx) => sqlite::$function(&mut **tx, $($argument),*).await,
            #[cfg(feature = "postgres")]
            Transaction::Postgres(tx) => postgres::$function(&mut **tx, $($argument),*).await,
        }
    };
}

/// Whether the device works offline, making no request, see `sync`.
pub const WORK_OFFLINE: &str = "work_offline";
/// Who is signed in on this device and the token the device sends the
/// server for them, a `users::SignedIn`, or null when nobody is: written
/// by a sign-in and cleared by a sign-out, see `accounts`.
pub const SIGNED_IN: &str = "signed_in";
/// The community the person works in on this device, an id, or null
/// for none chosen yet, see `current_community`.
pub const CURRENT_COMMUNITY: &str = "current_community_id";

/// The page the person was last on in this account, a path with its
/// query, or null for none yet, see `last_visited_page`.
pub const LAST_VISITED_PAGE: &str = "last_visited_page";

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
        (LAST_VISITED_PAGE, Value::Null),
        (LOCAL_USER_MOVED, Value::Null),
        (PLAN, Value::Null),
    ]
}

/// Why a setting could not be read or written.
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

/// Inserts each setting of `defaults` that is missing, with its first
/// value, and leaves the rest as the person set them. Returns how many
/// were inserted.
pub(crate) async fn insert_missing_defaults(tx: &mut Transaction) -> Result<usize, sqlx::Error> {
    let mut inserted = 0;
    for (key, value) in defaults() {
        if get_stored(&mut *tx, key).await?.is_none() {
            query!(&mut *tx, set(key, &value))?;
            inserted += 1;
        }
    }
    Ok(inserted)
}

/// The setting's stored JSON, or none when it was never set up.
pub async fn get_stored(tx: &mut Transaction, key: &str) -> Result<Option<Value>, sqlx::Error> {
    query!(tx, get(key))
}

/// The setting's value, which `upgrade` made sure is there.
pub async fn get<T: DeserializeOwned>(
    tx: &mut Transaction,
    key: &str,
) -> Result<T, DeviceSettingsError> {
    let value = get_stored(&mut *tx, key)
        .await?
        .ok_or_else(|| DeviceSettingsError::Missing(key.to_string()))?;
    serde_json::from_value(value).map_err(|source| DeviceSettingsError::Invalid {
        key: key.to_string(),
        source,
    })
}

/// Writes the setting's value.
pub async fn set<T: Serialize>(
    tx: &mut Transaction,
    key: &str,
    value: &T,
) -> Result<(), DeviceSettingsError> {
    let value = serde_json::to_value(value).map_err(|source| DeviceSettingsError::Invalid {
        key: key.to_string(),
        source,
    })?;
    Ok(query!(tx, set(key, &value))?)
}
