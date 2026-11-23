//! Preference records: the `Preference` schema, its baseline validation,
//! and the reads and writes of `accounts_preferences`, one row per user
//! and key. Callers own and commit the transaction. Writes are
//! crate-private: a preference syncs to its user's devices, so each
//! write goes with its event, see `accounts::preferences`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use crate::data::app_sync_state::Cursor;
use crate::database::Transaction;
use crate::helpers::locales;

pub const TABLE: &str = "accounts_preferences";

/// The language the pages are shown in: one of `locales::SUPPORTED`, or
/// null to follow the device's.
pub const LOCALE: &str = "locale";

/// Every key a preference may have.
pub const KEYS: &[&str] = &[LOCALE];

/// One of a person's choices, with the sync columns every synced row
/// carries, see `docs/design/offline-sync.md`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Preference {
    pub id: Uuid,
    pub user_id: Uuid,
    pub key: String,
    pub value: Value,
    pub inserted_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub version: i64,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub deleted_by_id: Option<Uuid>,
}

/// Why a preference may not be written.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PreferenceInvalid {
    #[error("unknown preference: {0}")]
    Key(String),
    #[error("not a value for {key}: {value}")]
    Value { key: String, value: String },
}

/// A preference that passed validation, which every write requires.
pub struct AcceptedPreference(Preference);

impl AcceptedPreference {
    /// Checks the preference's key and its value for that key.
    pub fn new(preference: Preference) -> Result<AcceptedPreference, PreferenceInvalid> {
        checked_value(&preference.key, &preference.value)?;
        Ok(AcceptedPreference(preference))
    }

    /// The accepted preference.
    pub fn row(&self) -> &Preference {
        &self.0
    }

    /// The accepted preference, owned.
    pub fn into_row(self) -> Preference {
        self.0
    }
}

/// Checks that the key is known and the value is one it takes.
pub fn checked_value(key: &str, value: &Value) -> Result<(), PreferenceInvalid> {
    let valid = match key {
        LOCALE => match value {
            Value::Null => true,
            Value::String(locale) => locales::is_supported(locale),
            _ => false,
        },
        _ => return Err(PreferenceInvalid::Key(key.to_string())),
    };
    if valid {
        Ok(())
    } else {
        Err(PreferenceInvalid::Value {
            key: key.to_string(),
            value: value.to_string(),
        })
    }
}

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

/// The user's live preferences.
pub async fn list(tx: &mut Transaction, user_id: Uuid) -> Result<Vec<Preference>, sqlx::Error> {
    query!(tx, list_by_user(user_id))
}

/// The user's preference for the key, deleted or not.
pub async fn get_by_user_and_key(
    tx: &mut Transaction,
    user_id: Uuid,
    key: &str,
) -> Result<Option<Preference>, sqlx::Error> {
    query!(tx, get_by_user_and_key(user_id, key))
}

/// Persists a row accepted by the resource workflow.
pub(crate) async fn create(
    tx: &mut Transaction,
    preference: &AcceptedPreference,
) -> Result<(), sqlx::Error> {
    query!(tx, insert(preference.row()))
}

/// Persists a row accepted by the resource workflow.
pub(crate) async fn update(
    tx: &mut Transaction,
    preference: &AcceptedPreference,
) -> Result<(), sqlx::Error> {
    query!(tx, update(preference.row()))
}

/// Explicit replication access for internal resource workflows.
pub mod replication {
    use super::*;

    /// Reads protocol state, including tombstones, in the caller transaction.
    pub async fn get(tx: &mut Transaction, id: Uuid) -> Result<Option<Preference>, sqlx::Error> {
        query!(tx, get(id))
    }

    /// The user's own rows the server wrote after the cursor: a
    /// preference is seen by its user alone.
    pub async fn list_synced_after(
        tx: &mut Transaction,
        user_id: Uuid,
        after: &Cursor,
        limit: usize,
    ) -> Result<Vec<Preference>, sqlx::Error> {
        query!(tx, list_synced_after(user_id, after, limit))
    }

    /// Persists the server's row as it is.
    pub(crate) async fn upsert(
        tx: &mut Transaction,
        preference: &Preference,
    ) -> Result<(), sqlx::Error> {
        query!(tx, upsert(preference))
    }

    /// Removes the row outright, as a device does with one the server refused.
    pub(crate) async fn delete(tx: &mut Transaction, id: Uuid) -> Result<(), sqlx::Error> {
        query!(tx, delete(id))
    }
}

/// Explicit maintenance access for internal resource workflows.
pub(crate) mod maintenance {
    use super::*;

    /// Deletes every preference of the user, with their account.
    pub(crate) async fn delete_of_user(
        tx: &mut Transaction,
        user_id: Uuid,
    ) -> Result<u64, sqlx::Error> {
        query!(tx, delete_of_user(user_id))
    }

    /// Deletes the preferences whose account is gone, a trial's or an
    /// unconfirmed sign-up's, which the purge removed; how many.
    pub(crate) async fn delete_without_account(tx: &mut Transaction) -> Result<u64, sqlx::Error> {
        query!(tx, delete_without_account())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn accounts_preferences_checked_value_takes_a_supported_locale_or_null() {
        assert_eq!(checked_value(LOCALE, &json!("en")), Ok(()));
        assert_eq!(checked_value(LOCALE, &Value::Null), Ok(()));
        assert!(matches!(
            checked_value(LOCALE, &json!("xx")),
            Err(PreferenceInvalid::Value { .. })
        ));
        assert!(matches!(
            checked_value(LOCALE, &json!(3)),
            Err(PreferenceInvalid::Value { .. })
        ));
        assert_eq!(
            checked_value("colour", &json!("red")),
            Err(PreferenceInvalid::Key("colour".to_string()))
        );
    }
}
