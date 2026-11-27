//! What a person chooses for themselves, the language and the theme the
//! pages are shown in: one row per user and key in
//! `accounts_preferences`, the one accounts table that syncs, to its own
//! user's devices only, so every app reads it, accounts included, and a
//! device changes it offline. Each write is an event, as a name is, and
//! the last one the server applies wins: a preference has no conflicts,
//! and two devices that each made the same key converge on the server's
//! row. See `docs/design/locales.md`.

use async_trait::async_trait;
use serde::Serialize;
use serde_json::{Value, json};
use thiserror::Error;
use uuid::Uuid;

use crate::data::accounts_preferences::{
    self, AcceptedPreference, LOCALE, Preference, PreferenceInvalid, TABLE, THEME,
};
use crate::data::app_events::{self, Conflicts, Event, EventKind, Fields};
use crate::data::app_users::User;
use crate::database::Transaction;
use crate::deployment::{Deployment, Role, now};
use crate::helpers::locales;
use crate::sync::TableRow;
use crate::sync::VersionRange;
use crate::sync::events::{self, Answer};
use crate::sync::synced_table::{LocalCreate, SyncedTable, TableError};

/// The fields a create sets.
const CREATE_FIELDS: &[&str] = &["user_id", "key", "value"];
/// The field an update may change.
const UPDATE_FIELDS: &[&str] = &["value"];

/// A person's preferences as the pages read them, each its default
/// when they made no choice.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Preferences {
    /// The language chosen, or none to follow the device's.
    pub locale: Option<String>,
    /// The theme chosen, or none to follow the device's light or dark setting.
    pub theme: Option<String>,
}

#[derive(Debug, Error)]
pub enum PreferenceError {
    #[error(transparent)]
    Invalid(#[from] PreferenceInvalid),
    #[error("the change was rejected: {0}")]
    Rejected(String),
    #[error(transparent)]
    Record(#[from] crate::sync::events::RecordError),
    #[error("{0}")]
    Json(#[from] serde_json::Error),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// The user's preferences. A row whose value no longer applies, a
/// language since dropped say, reads as its default.
pub async fn get(deployment: &Deployment, user: &User) -> Result<Preferences, PreferenceError> {
    let mut tx = deployment.begin().await?;
    let preferences = get_in(&mut tx, user.id).await?;
    tx.commit().await?;
    Ok(preferences)
}

/// The user's preferences, in the caller's transaction.
pub async fn get_in(tx: &mut Transaction, user_id: Uuid) -> Result<Preferences, PreferenceError> {
    let mut preferences = Preferences::default();
    for row in accounts_preferences::list(tx, user_id).await? {
        if accounts_preferences::checked_value(&row.key, &row.value).is_err() {
            continue;
        }
        let value = row.value.as_str().map(str::to_string);
        match row.key.as_str() {
            LOCALE => preferences.locale = value,
            THEME => preferences.theme = value,
            _ => {}
        }
    }
    Ok(preferences)
}

/// The language to mail the user in: the one they chose, else
/// `requested`, the language the request asked for, else English. With no
/// user yet, a sign-up's, `requested`.
pub(crate) async fn mail_locale_in(
    tx: &mut Transaction,
    user_id: Option<Uuid>,
    requested: &str,
) -> Result<&'static str, sqlx::Error> {
    let chosen = match user_id {
        Some(user_id) => accounts_preferences::get_by_user_and_key(tx, user_id, LOCALE)
            .await?
            .and_then(|row| row.value.as_str().map(str::to_string)),
        None => None,
    };
    let locale = chosen.as_deref().unwrap_or(requested);
    Ok(crate::app::locales::SUPPORTED
        .iter()
        .copied()
        .find(|supported| *supported == locale)
        .unwrap_or(locales::DEFAULT))
}

/// Sets one of the user's preferences, with the time from this
/// deployment: an event like any change a person makes, so a device
/// writes it offline and the next sync carries it.
pub async fn update(
    deployment: &Deployment,
    user: &User,
    key: &str,
    value: Value,
) -> Result<Preferences, PreferenceError> {
    accounts_preferences::checked_value(key, &value)?;
    let mut tx = deployment.begin().await?;
    let event = match accounts_preferences::get_by_user_and_key(&mut tx, user.id, key).await? {
        Some(row) if row.value == value => {
            tx.commit().await?;
            return get(deployment, user).await;
        }
        Some(row) => Event::update(
            TABLE,
            row.id,
            row.version,
            Fields::from_iter([("value".to_string(), row.value)]),
            Fields::from_iter([("value".to_string(), value)]),
            now(),
        ),
        None => Event::create(
            TABLE,
            Uuid::now_v7(),
            Fields::from_iter([
                ("user_id".to_string(), json!(user.id)),
                ("key".to_string(), json!(key)),
                ("value".to_string(), value),
            ]),
            now(),
        ),
    };
    let answer = apply_in(&mut tx, deployment.role, user, &event).await?;
    if let Some(reason) = answer.reason {
        return Err(PreferenceError::Rejected(reason));
    }
    let preferences = get_in(&mut tx, user.id).await?;
    tx.commit().await?;
    Ok(preferences)
}

/// Applies the event in the transaction. A create for a key the user
/// already has writes its value over that row, which the answer carries,
/// so the device's row under the other id goes. Only the user's own rows
/// change, and only to a value their key takes.
pub(crate) async fn apply_in(
    tx: &mut Transaction,
    role: Role,
    user: &User,
    event: &Event,
) -> Result<Answer, PreferenceError> {
    crate::data::app_sync_versions::lock_for_role(tx, role).await?;
    if role == Role::Server
        && let Some(recorded) = app_events::get(tx, event.id).await?
    {
        let row = accounts_preferences::replication::get(tx, event.row_id).await?;
        let server = row.as_ref().map(table_row).transpose()?;
        return Ok(recorded.answer(server));
    }
    let allowed = match event.kind {
        EventKind::Create => CREATE_FIELDS,
        EventKind::Update => UPDATE_FIELDS,
    };
    if let Some(reason) = events::not_editable(&event.changes, allowed) {
        let row = accounts_preferences::replication::get(tx, event.row_id).await?;
        return rejected(event, reason, row.as_ref());
    }
    let Some(value) = event.changes.get("value").cloned() else {
        let row = accounts_preferences::replication::get(tx, event.row_id).await?;
        return rejected(event, "invalid: a value is needed", row.as_ref());
    };
    let row = match event.kind {
        EventKind::Create => {
            let user_id = event.changes.get("user_id").and_then(Value::as_str);
            if user_id != Some(user.id.to_string().as_str()) {
                return rejected(event, "not_yours", None);
            }
            let Some(key) = event.changes.get("key").and_then(Value::as_str) else {
                return rejected(event, "invalid: a key is needed", None);
            };
            match accounts_preferences::get_by_user_and_key(tx, user.id, key).await? {
                Some(existing) => existing,
                None => {
                    if let Some(other) =
                        accounts_preferences::replication::get(tx, event.row_id).await?
                    {
                        return rejected(event, "exists", Some(&other));
                    }
                    return create(tx, role, user, event, key, value).await;
                }
            }
        }
        EventKind::Update => {
            let Some(row) = accounts_preferences::replication::get(tx, event.row_id).await? else {
                return rejected(event, "unknown_preference", None);
            };
            if row.user_id != user.id {
                return rejected(event, "not_yours", None);
            }
            row
        }
    };
    if row.value == value {
        return finish(tx, role, user, event, &row).await;
    }
    let now = now();
    let mut updated = Preference {
        value,
        updated_at: now,
        ..row.clone()
    };
    if role == Role::Server {
        updated.version += 1;
        updated.last_synced_at = Some(now);
    }
    let mut updated = match AcceptedPreference::new(updated) {
        Ok(updated) => updated,
        Err(invalid) => return rejected(event, format!("invalid: {invalid}"), Some(&row)),
    };
    updated.record_sync_change(tx, role).await?;
    accounts_preferences::update(tx, &updated).await?;
    finish(tx, role, user, event, updated.row()).await
}

async fn create(
    tx: &mut Transaction,
    role: Role,
    user: &User,
    event: &Event,
    key: &str,
    value: Value,
) -> Result<Answer, PreferenceError> {
    let now = now();
    let (version, last_synced_at) = match role {
        Role::Server => (1, Some(now)),
        Role::Device => (0, None),
    };
    let preference = Preference {
        id: event.row_id,
        user_id: user.id,
        key: key.to_string(),
        value,
        inserted_at: event.row_inserted_at(now),
        updated_at: now,
        version,
        last_synced_at,
        global_sync_version: None,
        deleted_at: None,
        deleted_by_id: None,
    };
    let mut preference = match AcceptedPreference::new(preference) {
        Ok(preference) => preference,
        Err(invalid) => return rejected(event, format!("invalid: {invalid}"), None),
    };
    preference.record_sync_change(tx, role).await?;
    accounts_preferences::create(tx, &preference).await?;
    finish(tx, role, user, event, preference.row()).await
}

/// Answers an applied event and does the role's part.
async fn finish(
    tx: &mut Transaction,
    role: Role,
    user: &User,
    event: &Event,
    row: &Preference,
) -> Result<Answer, PreferenceError> {
    let answer = Answer::applied(event.id, row.version, Conflicts::new(), table_row(row)?);
    match role {
        Role::Server => events::record(tx, user.id, event, &answer).await?,
        Role::Device => events::queue(tx, user.id, event).await?,
    }
    Ok(answer)
}

fn rejected(
    event: &Event,
    reason: impl Into<String>,
    row: Option<&Preference>,
) -> Result<Answer, PreferenceError> {
    let server = row.map(table_row).transpose()?;
    Ok(Answer::rejected(event.id, reason, server))
}

/// The preference as it travels on the wire.
pub fn table_row(preference: &Preference) -> Result<TableRow, serde_json::Error> {
    Ok(TableRow {
        table: TABLE.to_string(),
        row: serde_json::to_value(preference)?,
    })
}

/// Deletes every preference of the user, with their account.
pub(crate) async fn delete_of_user_in(
    tx: &mut Transaction,
    user_id: Uuid,
) -> Result<(), sqlx::Error> {
    accounts_preferences::maintenance::delete_of_user(tx, user_id).await?;
    Ok(())
}

/// Deletes the preferences of accounts the purge removed; how many.
pub(crate) async fn delete_without_account_in(tx: &mut Transaction) -> Result<u64, sqlx::Error> {
    accounts_preferences::maintenance::delete_without_account(tx).await
}

/// Writes the server's row over the device's, unless an event of the
/// device's for it is still pending. The device's own row for the key
/// under another id goes first, unless it waits to be pushed, whose
/// answer then brings the server's.
async fn apply_server_row(tx: &mut Transaction, row: &Value) -> Result<bool, PreferenceError> {
    let preference: Preference = serde_json::from_value(row.clone())?;
    if app_events::has_pending(tx, TABLE, preference.id).await? {
        return Ok(false);
    }
    let mine =
        accounts_preferences::get_by_user_and_key(tx, preference.user_id, &preference.key).await?;
    if let Some(mine) = mine.filter(|mine| mine.id != preference.id) {
        if app_events::has_pending(tx, TABLE, mine.id).await? {
            return Ok(false);
        }
        accounts_preferences::replication::delete(tx, mine.id).await?;
    }
    accounts_preferences::replication::upsert(tx, &preference).await?;
    Ok(true)
}

/// Preferences as a synced table, see `app::synced_tables`.
pub struct Synced;

#[async_trait]
impl SyncedTable for Synced {
    fn table(&self) -> &'static str {
        TABLE
    }

    fn owner_column(&self) -> Option<&'static str> {
        Some("user_id")
    }

    async fn apply_in(
        &self,
        tx: &mut Transaction,
        role: Role,
        user: &User,
        event: &Event,
    ) -> Result<Answer, TableError> {
        Ok(apply_in(tx, role, user, event).await?)
    }

    async fn list_synced_after(
        &self,
        tx: &mut Transaction,
        user: &User,
        range: &VersionRange,
        limit: usize,
    ) -> Result<Vec<TableRow>, TableError> {
        let rows =
            accounts_preferences::replication::list_synced_after(tx, user.id, range, limit).await?;
        Ok(rows.iter().map(table_row).collect::<Result<_, _>>()?)
    }

    /// A preference points at its user, whose own row a device always has.
    async fn parents(
        &self,
        _tx: &mut Transaction,
        _row: &TableRow,
    ) -> Result<Vec<TableRow>, TableError> {
        Ok(Vec::new())
    }

    async fn apply_server_row(
        &self,
        tx: &mut Transaction,
        _user: &User,
        row: &TableRow,
    ) -> Result<bool, TableError> {
        Ok(apply_server_row(tx, &row.row).await?)
    }

    async fn delete_row(&self, tx: &mut Transaction, id: Uuid) -> Result<(), TableError> {
        Ok(accounts_preferences::replication::delete(tx, id).await?)
    }

    async fn local_user_creates(
        &self,
        tx: &mut Transaction,
        user: &User,
    ) -> Result<Vec<LocalCreate>, TableError> {
        let rows = accounts_preferences::list(tx, user.id).await?;
        Ok(rows
            .into_iter()
            .map(|row| LocalCreate {
                id: row.id,
                inserted_at: row.inserted_at,
                parents: Vec::new(),
                changes: Fields::from_iter([
                    ("user_id".to_string(), json!(row.user_id)),
                    ("key".to_string(), json!(row.key)),
                    ("value".to_string(), row.value),
                ]),
            })
            .collect())
    }

    async fn label(&self, tx: &mut Transaction, id: Uuid) -> Result<Option<String>, TableError> {
        let row = accounts_preferences::replication::get(tx, id).await?;
        Ok(row.map(|row| row.key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts;
    use crate::app::{local_users, users};
    use crate::database::{Database, with_each_database};
    use crate::services::web_server::InProcess;
    use crate::sync;

    fn server(database: Database) -> Deployment {
        Deployment::new(database, Role::Server)
    }

    async fn rows_of(deployment: &Deployment, user_id: Uuid) -> Vec<Preference> {
        let mut tx = deployment.begin().await.expect("a transaction");
        accounts_preferences::list(&mut tx, user_id)
            .await
            .expect("the rows")
    }

    async fn synced(device: &Deployment, server: &Deployment) {
        sync::run(device, &InProcess { server })
            .await
            .expect("a sync");
    }

    #[tokio::test]
    async fn preferences_update_on_the_server_writes_one_row_per_key_and_reads_it_back() {
        with_each_database(|database| async move {
            let server = server(database);
            let alice = accounts::tests::signed_up_for_a_device(&server, "alice@example.com")
                .await
                .user;
            assert_eq!(
                get(&server, &alice).await.expect("read"),
                Preferences::default()
            );

            let chosen = update(&server, &alice, LOCALE, json!("en"))
                .await
                .expect("chosen");
            assert_eq!(chosen.locale.as_deref(), Some("en"));
            let followed = update(&server, &alice, LOCALE, Value::Null)
                .await
                .expect("the device's again");
            assert_eq!(followed.locale, None);

            let rows = rows_of(&server, alice.id).await;
            assert_eq!(rows.len(), 1, "one row, written over");
            assert_eq!(rows[0].version, 2);
            assert!(rows[0].last_synced_at.is_some());
        })
        .await;
    }

    #[tokio::test]
    async fn preferences_update_keeps_the_theme_beside_the_locale() {
        with_each_database(|database| async move {
            let server = server(database);
            let alice = accounts::tests::signed_up_for_a_device(&server, "alice@example.com")
                .await
                .user;
            update(&server, &alice, LOCALE, json!("es"))
                .await
                .expect("a language");
            let chosen = update(&server, &alice, THEME, json!("dracula"))
                .await
                .expect("a theme");
            assert_eq!(chosen.theme.as_deref(), Some("dracula"));
            assert_eq!(chosen.locale.as_deref(), Some("es"));
            assert_eq!(rows_of(&server, alice.id).await.len(), 2);
        })
        .await;
    }

    #[tokio::test]
    async fn preferences_update_refuses_an_unknown_key_or_a_value_the_key_does_not_take() {
        with_each_database(|database| async move {
            let server = server(database);
            let alice = accounts::tests::signed_up_for_a_device(&server, "alice@example.com")
                .await
                .user;
            assert!(matches!(
                update(&server, &alice, "colour", json!("red")).await,
                Err(PreferenceError::Invalid(PreferenceInvalid::Key(_)))
            ));
            assert!(matches!(
                update(&server, &alice, LOCALE, json!("xx")).await,
                Err(PreferenceError::Invalid(PreferenceInvalid::Value { .. }))
            ));
            assert!(rows_of(&server, alice.id).await.is_empty());
        })
        .await;
    }

    #[tokio::test]
    async fn preferences_sync_to_the_users_other_devices_and_to_nobody_else() {
        with_each_database(|database| async move {
            let server = server(database);
            let alice = accounts::tests::signed_up_for_a_device(&server, "alice@example.com").await;
            let bob = accounts::tests::signed_up_for_a_device(&server, "bob@example.com").await;
            let laptop = users::tests::device_of(&alice.user, &alice.token).await;
            let phone = users::tests::device_of(&alice.user, &alice.token).await;
            let bobs = users::tests::device_of(&bob.user, &bob.token).await;

            update(&laptop, &alice.user, LOCALE, json!("en"))
                .await
                .expect("chosen offline");
            assert_eq!(rows_of(&laptop, alice.user.id).await[0].version, 0);
            synced(&laptop, &server).await;
            synced(&phone, &server).await;
            synced(&bobs, &server).await;

            let on_phone = get(&phone, &alice.user).await.expect("read");
            assert_eq!(on_phone.locale.as_deref(), Some("en"));
            assert!(
                rows_of(&bobs, alice.user.id).await.is_empty(),
                "never Bob's"
            );
            let on_server = get(&server, &alice.user).await.expect("read");
            assert_eq!(on_server.locale.as_deref(), Some("en"));
        })
        .await;
    }

    #[tokio::test]
    async fn preferences_made_on_two_devices_offline_converge_on_the_servers_row() {
        with_each_database(|database| async move {
            let server = server(database);
            let alice = accounts::tests::signed_up_for_a_device(&server, "alice@example.com").await;
            let laptop = users::tests::device_of(&alice.user, &alice.token).await;
            let phone = users::tests::device_of(&alice.user, &alice.token).await;
            update(&laptop, &alice.user, LOCALE, json!("en"))
                .await
                .expect("on the laptop");
            update(&phone, &alice.user, LOCALE, Value::Null)
                .await
                .expect("on the phone, later");

            synced(&laptop, &server).await;
            synced(&phone, &server).await;
            synced(&laptop, &server).await;

            let kept = rows_of(&server, alice.user.id).await;
            assert_eq!(kept.len(), 1);
            assert_eq!(
                kept[0].value,
                Value::Null,
                "the last one the server applied"
            );
            for device in [&laptop, &phone] {
                let rows = rows_of(device, alice.user.id).await;
                assert_eq!(rows.len(), 1);
                assert_eq!(rows[0].id, kept[0].id, "the server's row on both");
                assert_eq!(rows[0].value, Value::Null);
                assert_eq!(sync::status(device).await.expect("status").pending, 0);
            }
        })
        .await;
    }

    #[tokio::test]
    async fn preferences_apply_refuses_a_change_to_another_users_row() {
        with_each_database(|database| async move {
            let server = server(database);
            let alice = accounts::tests::signed_up_for_a_device(&server, "alice@example.com")
                .await
                .user;
            let bob = accounts::tests::signed_up_for_a_device(&server, "bob@example.com")
                .await
                .user;
            update(&server, &alice, LOCALE, json!("en"))
                .await
                .expect("chosen");
            let row = rows_of(&server, alice.id).await.remove(0);

            let event = Event::update(
                TABLE,
                row.id,
                row.version,
                Fields::from_iter([("value".to_string(), json!("en"))]),
                Fields::from_iter([("value".to_string(), Value::Null)]),
                now(),
            );
            let mut tx = server.begin().await.expect("a transaction");
            let answer = apply_in(&mut tx, Role::Server, &bob, &event)
                .await
                .expect("answered");
            assert_eq!(answer.reason.as_deref(), Some("not_yours"));
            let create = Event::create(
                TABLE,
                Uuid::now_v7(),
                Fields::from_iter([
                    ("user_id".to_string(), json!(alice.id)),
                    ("key".to_string(), json!(LOCALE)),
                    ("value".to_string(), Value::Null),
                ]),
                now(),
            );
            let answer = apply_in(&mut tx, Role::Server, &bob, &create)
                .await
                .expect("answered");
            assert_eq!(answer.reason.as_deref(), Some("not_yours"));
            tx.commit().await.expect("committed");
            assert_eq!(rows_of(&server, alice.id).await[0].value, json!("en"));
        })
        .await;
    }

    #[tokio::test]
    async fn preferences_delete_without_account_removes_only_those_whose_account_is_gone() {
        with_each_database(|database| async move {
            let server = server(database);
            let alice = accounts::tests::signed_up_for_a_device(&server, "alice@example.com")
                .await
                .user;
            update(&server, &alice, LOCALE, json!("en"))
                .await
                .expect("chosen");
            let gone = User {
                id: Uuid::now_v7(),
                ..alice.clone()
            };
            let mut tx = server.begin().await.expect("a transaction");
            create(
                &mut tx,
                Role::Server,
                &gone,
                &Event::create(TABLE, Uuid::now_v7(), Fields::new(), now()),
                LOCALE,
                json!("en"),
            )
            .await
            .expect("an orphan's row");
            let removed = delete_without_account_in(&mut tx).await.expect("swept");
            tx.commit().await.expect("committed");
            assert_eq!(removed, 1);
            assert_eq!(rows_of(&server, alice.id).await.len(), 1);
            assert!(rows_of(&server, gone.id).await.is_empty());
        })
        .await;
    }

    #[tokio::test]
    async fn preferences_mail_locale_in_is_the_choice_else_the_request_else_english() {
        with_each_database(|database| async move {
            let server = server(database);
            let alice = accounts::tests::signed_up_for_a_device(&server, "alice@example.com")
                .await
                .user;
            let mut tx = server.begin().await.expect("a transaction");
            let asked = mail_locale_in(&mut tx, Some(alice.id), "es")
                .await
                .expect("asked");
            assert_eq!(asked, "es", "the request's, with no choice");
            let unknown = mail_locale_in(&mut tx, None, "xx").await.expect("asked");
            assert_eq!(
                unknown,
                locales::DEFAULT,
                "English for a language the app is not in"
            );
            tx.commit().await.expect("committed");

            update(&server, &alice, LOCALE, json!("es"))
                .await
                .expect("chosen");
            let mut tx = server.begin().await.expect("a transaction");
            let chosen = mail_locale_in(&mut tx, Some(alice.id), "en")
                .await
                .expect("asked");
            assert_eq!(chosen, "es", "her choice over the request's");
        })
        .await;
    }

    #[tokio::test]
    async fn preferences_of_a_local_user_move_with_them_to_their_account() {
        with_each_database(|database| async move {
            let server = server(database);
            let device = users::tests::device().await;
            let local =
                local_users::create(&device, Uuid::now_v7(), &local_users::new_name(), "Laptop")
                    .await
                    .expect("a local user");
            update(&device, &local, LOCALE, json!("en"))
                .await
                .expect("chosen before signing up");
            assert_eq!(
                sync::status(&device).await.expect("status").pending,
                0,
                "never queued"
            );

            let session =
                accounts::tests::signed_up_for_a_device(&server, "alice@example.com").await;
            let alice = local_users::move_to_account(&device, session, None)
                .await
                .expect("moved");
            synced(&device, &server).await;

            let on_server = get(&server, &alice).await.expect("read");
            assert_eq!(on_server.locale.as_deref(), Some("en"));
            assert!(rows_of(&device, local.id).await.is_empty());
        })
        .await;
    }
}
