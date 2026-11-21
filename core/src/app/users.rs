//! Users: the app's view of each person, their name and picture, one
//! row per person, written by the server at sign-up and synced to
//! devices like any row, so a post can show who wrote it. Their email
//! is their account's, `accounts::users`, a row with the same id that
//! never leaves the server, as their password, `credentials`, and
//! tokens, `tokens`, do not; the flows are in `accounts`. Every core function takes
//! the `User` acting: the server resolves it from the request's token,
//! `get_by_token`, and a device keeps who is signed in as a device
//! setting, `signed_in`. Nobody acts until someone signs in.

use crate::data::app_users::{self, AcceptedUser, MAX_NAME_LENGTH, TABLE, User, UserInvalid};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;
use uuid::Uuid;

use crate::app::events::{self, Answer, AnswerStatus, Conflicts, Event, EventKind, Fields};
use crate::app::sync::{Cursor, TableRow};
use crate::data::accounts_tokens;
use crate::data::accounts_users::{self, AccountUserError};
use crate::data::device_settings::{self, DeviceSettingsError, SIGNED_IN};
use crate::database::Transaction;
use crate::deployment::{Deployment, Role, now};

/// What a person changes of their own row; the rest is the server's.
pub(crate) const UPDATE_FIELDS: &[&str] = &["name"];

/// Who is signed in on a device, and the token the device sends the
/// server for them: the `signed_in` device setting, written by a
/// sign-in and cleared by a sign-out, see `accounts`. A local user has
/// no token, having no account on the server, see `local_users`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedIn {
    pub user_id: Uuid,
    #[serde(default)]
    pub token: Option<String>,
}

/// What kind of user someone is, for the few places that must tell:
/// the pages read it from the current user.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// Signed up, with an account on the server.
    Full,
    /// Made on a desktop or phone without signing up, on that device
    /// only, see `local_users`.
    Local,
    /// Made on the web without signing up, and deleted soon after
    /// unless they sign up, see `trial_users`.
    Trial,
}

#[derive(Debug, Error)]
pub enum UserError {
    #[error("user {id} not found")]
    NotFound { id: Uuid },
    #[error(transparent)]
    Validation(#[from] UserInvalid),
    #[error(transparent)]
    AccountUser(#[from] AccountUserError),
    #[error("only the server creates users")]
    ServerOnly,
    #[error("The server did not accept this token")]
    UnknownToken,
    /// The token's account was deleted, see `accounts::deletions`.
    #[error("This account was deleted")]
    AccountDeleted,
    #[error("Nobody is signed in")]
    NotSignedIn,
    #[error("This needs an account: sign up or sign in first")]
    LocalUser,
    #[error("the change was rejected: {0}")]
    Rejected(String),
    #[error("{0}")]
    Invalid(String),
    #[error(transparent)]
    DeviceSettings(#[from] DeviceSettingsError),
    #[error(transparent)]
    Record(#[from] crate::app::events::RecordError),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl From<serde_json::Error> for UserError {
    fn from(error: serde_json::Error) -> UserError {
        UserError::Invalid(error.to_string())
    }
}

/// One user by their id, unless deleted.
pub async fn get(deployment: &Deployment, id: Uuid) -> Result<User, UserError> {
    let mut tx = deployment.begin().await?;
    let user = get_live(&mut tx, id).await?;
    tx.commit().await?;
    Ok(user)
}

/// Makes a user on the server with their account, its email taken as
/// proved: plain server writes, never an event, since a device never
/// creates one: for a seed or a test. A sign-up goes through
/// `accounts::sign_up`, whose account is unconfirmed until its code
/// matches.
pub async fn create(deployment: &Deployment, email: &str, name: &str) -> Result<User, UserError> {
    if deployment.role != Role::Server {
        return Err(UserError::ServerOnly);
    }
    let mut tx = deployment.begin().await?;
    let id = Uuid::now_v7();
    accounts_users::create(&mut tx, id, email, true).await?;
    let user = create_in(&mut tx, id, name).await?;
    tx.commit().await?;
    Ok(user)
}

/// The user's row, with the id of their account and their name as
/// `checked_name` keeps it.
pub(crate) async fn create_in(
    tx: &mut Transaction,
    id: Uuid,
    name: &str,
) -> Result<User, UserError> {
    let now = now();
    let user = User {
        id,
        name: name.to_string(),
        inserted_at: now,
        updated_at: now,
        version: 1,
        last_synced_at: Some(now),
        deleted_at: None,
        deleted_by_id: None,
        avatar_url: None,
    };
    let user = AcceptedUser::new(user)?;
    app_users::create(&mut *tx, &user).await?;
    let user = user.into_row();
    Ok(user)
}

/// Writes a new name over the user's own row, with the time from this
/// deployment: an event like any change a person makes, so a device
/// writes it offline and the next sync carries it, see
/// `docs/design/offline-sync.md`.
pub async fn update(deployment: &Deployment, user: &User, name: &str) -> Result<User, UserError> {
    let name = app_users::checked_name(name)?;
    let mut tx = deployment.begin().await?;
    let current = get_live(&mut tx, user.id).await?;
    if current.name == name {
        return Ok(current);
    }
    let event = Event::update(
        TABLE,
        current.id,
        current.version,
        Fields::from_iter([("name".to_string(), json!(current.name))]),
        Fields::from_iter([("name".to_string(), json!(name))]),
        now(),
    );
    let user = apply_in(&mut tx, deployment.role, &current, &event)
        .await?
        .user()?;
    tx.commit().await?;
    Ok(user)
}

/// Applies an event of the user's to their own row and answers it: what
/// the server does with a pushed event.
pub async fn apply_event(
    deployment: &Deployment,
    user: &User,
    event: &Event,
) -> Result<Answer, UserError> {
    let mut tx = deployment.begin().await?;
    let applied = apply_in(&mut tx, deployment.role, user, event).await?;
    tx.commit().await?;
    Ok(applied.answer)
}

/// An event applied: the answer, and the user as they now stand.
pub(crate) struct Applied {
    pub answer: Answer,
    pub user: Option<User>,
}

impl Applied {
    fn user(self) -> Result<User, UserError> {
        match (self.answer.status, self.user) {
            (AnswerStatus::Rejected, _) => Err(UserError::Rejected(
                self.answer.reason.unwrap_or_else(|| "rejected".to_string()),
            )),
            (_, Some(user)) => Ok(user),
            (_, None) => Err(UserError::NotFound { id: self.answer.id }),
        }
    }
}

/// Applies the event in the transaction: an update of the user's own
/// row merges into it as a post's does. Only the server makes a user,
/// so a create is rejected, and so is a change to anyone else.
pub(crate) async fn apply_in(
    tx: &mut Transaction,
    role: Role,
    user: &User,
    event: &Event,
) -> Result<Applied, UserError> {
    if role == Role::Server
        && let Some(recorded) = events::get(tx, event.id).await?
    {
        let row = get_in(tx, event.row_id).await?;
        let server = row.as_ref().map(table_row).transpose()?;
        return Ok(Applied {
            answer: recorded.answer(server),
            user: row,
        });
    }
    let Some(row) = get_in(tx, event.row_id).await? else {
        return rejected(event, "unknown_user", None);
    };
    if event.kind != EventKind::Update {
        return rejected(event, "server_only", Some(row));
    }
    if row.id != user.id {
        return rejected(event, "not_yours", Some(row));
    }
    if let Some(reason) = events::not_editable(&event.changes, UPDATE_FIELDS) {
        return rejected(event, reason, Some(row));
    }
    let current = Fields::from_iter([("name".to_string(), json!(row.name))]);
    let (changes, conflicts) = events::merge(
        &current,
        row.version,
        event.base_version,
        &event.base,
        &event.changes,
    );
    let Some(name) = changes.get("name") else {
        return finish(tx, role, user, event, conflicts, row).await;
    };
    let Some(name) = name
        .as_str()
        .and_then(|name| app_users::checked_name(name).ok())
    else {
        return rejected(event, "invalid: a name is needed", Some(row));
    };
    let now = now();
    let mut updated = User {
        name,
        updated_at: now,
        ..row
    };
    if role == Role::Server {
        updated.version += 1;
        updated.last_synced_at = Some(now);
    }
    let updated = AcceptedUser::new(updated)?;
    app_users::update(&mut *tx, &updated).await?;
    let updated = updated.into_row();
    finish(tx, role, user, event, conflicts, updated).await
}

/// Answers an applied event and does the role's part.
async fn finish(
    tx: &mut Transaction,
    role: Role,
    user: &User,
    event: &Event,
    conflicts: Conflicts,
    row: User,
) -> Result<Applied, UserError> {
    let answer = Answer::applied(event.id, row.version, conflicts, table_row(&row)?);
    match role {
        Role::Server => events::record(tx, user.id, event, &answer).await?,
        Role::Device => events::queue(tx, user.id, event).await?,
    }
    Ok(Applied {
        answer,
        user: Some(row),
    })
}

fn rejected(
    event: &Event,
    reason: impl Into<String>,
    row: Option<User>,
) -> Result<Applied, UserError> {
    let server = row.as_ref().map(table_row).transpose()?;
    Ok(Applied {
        answer: Answer::rejected(event.id, reason, server),
        user: row,
    })
}

/// Sets the user's name, a server write the devices pull like any
/// other, for a trial user signing up.
pub(crate) async fn update_name_in(
    tx: &mut Transaction,
    id: Uuid,
    name: &str,
) -> Result<User, UserError> {
    let user = get_live(tx, id).await?;
    let now = now();
    let updated = User {
        name: name.to_string(),
        updated_at: now,
        version: user.version + 1,
        last_synced_at: Some(now),
        ..user
    };
    let updated = AcceptedUser::new(updated)?;
    app_users::update(&mut *tx, &updated).await?;
    let updated = updated.into_row();
    Ok(updated)
}

/// Sets the user's picture, on the server, as an event in its log like
/// any change a person makes; the devices pull the row. A device never
/// writes it, so there is no merge, see `avatars`.
#[cfg_attr(not(feature = "object-storage"), allow(dead_code))]
pub(crate) async fn update_avatar_url_in(
    tx: &mut Transaction,
    id: Uuid,
    avatar_url: &str,
) -> Result<User, UserError> {
    let user = get_live(tx, id).await?;
    let now = now();
    let event = Event::update(
        TABLE,
        id,
        user.version,
        Fields::from_iter([("avatar_url".to_string(), json!(user.avatar_url))]),
        Fields::from_iter([("avatar_url".to_string(), json!(avatar_url))]),
        now,
    );
    let updated = User {
        avatar_url: Some(avatar_url.to_string()),
        updated_at: now,
        version: user.version + 1,
        last_synced_at: Some(now),
        ..user
    };
    let updated = AcceptedUser::new(updated)?;
    app_users::update(&mut *tx, &updated).await?;
    let updated = updated.into_row();
    let answer = Answer::applied(
        event.id,
        updated.version,
        Conflicts::new(),
        table_row(&updated)?,
    );
    events::record(tx, id, &event, &answer).await?;
    Ok(updated)
}

/// What a deleted user is shown as, to those who still see their row.
pub const DELETED_NAME: &str = "Deleted user";

/// Deletes the user, on the server, as an event in its log: the row
/// soft-deleted, named `DELETED_NAME` and without a picture, so every
/// device that pulled it sees them go, see `accounts::deletions`.
pub(crate) async fn delete_in(tx: &mut Transaction, id: Uuid) -> Result<User, UserError> {
    let user = get_live(tx, id).await?;
    let now = now();
    let event = Event::update(
        TABLE,
        id,
        user.version,
        Fields::from_iter([
            ("name".to_string(), json!(user.name)),
            ("avatar_url".to_string(), json!(user.avatar_url)),
            ("deleted_at".to_string(), json!(null)),
        ]),
        Fields::from_iter([
            ("name".to_string(), json!(DELETED_NAME)),
            ("avatar_url".to_string(), json!(null)),
            ("deleted_at".to_string(), json!(now)),
        ]),
        now,
    );
    let deleted = User {
        name: DELETED_NAME.to_string(),
        avatar_url: None,
        updated_at: now,
        version: user.version + 1,
        last_synced_at: Some(now),
        deleted_at: Some(now),
        deleted_by_id: Some(id),
        ..user
    };
    let deleted = AcceptedUser::new(deleted)?;
    app_users::delete(&mut *tx, &deleted).await?;
    let deleted = deleted.into_row();
    let answer = Answer::applied(
        event.id,
        deleted.version,
        Conflicts::new(),
        table_row(&deleted)?,
    );
    events::record(tx, id, &event, &answer).await?;
    Ok(deleted)
}

/// Every user's picture, deleted users' too, since a deleted row keeps
/// its file until the purge removes the row.
#[cfg_attr(not(feature = "object-storage"), allow(dead_code))]
pub(crate) async fn list_avatar_urls_in(tx: &mut Transaction) -> Result<Vec<String>, UserError> {
    Ok(app_users::maintenance::list_avatar_urls(tx).await?)
}

/// Deletes the users whose account never proved its email and was made
/// before the time, for the purge, ahead of the accounts themselves.
pub(crate) async fn delete_of_unconfirmed_accounts_before(
    tx: &mut Transaction,
    time: DateTime<Utc>,
) -> Result<(), UserError> {
    Ok(app_users::maintenance::delete_of_unconfirmed_accounts_before(&mut *tx, time).await?)
}

/// The live user the token stands for, on the server; a token the
/// server does not know, or one that has expired, stands for nobody,
/// and one of a deleted account is `AccountDeleted`.
pub async fn get_by_token(deployment: &Deployment, token: &str) -> Result<User, UserError> {
    let (api_made_after, session_made_after) = accounts_tokens::made_after_to_be_valid(now());
    let mut tx = deployment.begin().await?;
    let user = app_users::authentication::get_by_token_hash(
        &mut tx,
        &accounts_tokens::hash(token),
        api_made_after,
        session_made_after,
    )
    .await?;
    tx.commit().await?;
    match user {
        Some(user) if user.deleted_at.is_some() => Err(UserError::AccountDeleted),
        Some(user) => Ok(user),
        None => Err(UserError::UnknownToken),
    }
}

/// The user signed in on this device, a device setting; a local user
/// too.
pub async fn signed_in(deployment: &Deployment) -> Result<User, UserError> {
    let mut tx = deployment.begin().await?;
    let signed_in: Option<SignedIn> = device_settings::get(&mut tx, SIGNED_IN).await?;
    let signed_in = signed_in.ok_or(UserError::NotSignedIn)?;
    let user = get_live(&mut tx, signed_in.user_id).await?;
    tx.commit().await?;
    Ok(user)
}

/// The user signed in on this device, and the token the device sends
/// the server for them; a local user has none, and whatever asks the
/// server is refused them.
pub async fn signed_in_with_token(deployment: &Deployment) -> Result<(User, String), UserError> {
    let mut tx = deployment.begin().await?;
    let signed_in: Option<SignedIn> = device_settings::get(&mut tx, SIGNED_IN).await?;
    let signed_in = signed_in.ok_or(UserError::NotSignedIn)?;
    let token = signed_in.token.ok_or(UserError::LocalUser)?;
    let user = get_live(&mut tx, signed_in.user_id).await?;
    tx.commit().await?;
    Ok((user, token))
}

/// The user as they travel on the wire.
pub fn table_row(user: &User) -> Result<TableRow, UserError> {
    Ok(TableRow {
        table: TABLE.to_string(),
        row: serde_json::to_value(user)?,
    })
}

/// Writes the server's row over the device's, unless an event of the
/// device's for it is still pending; true when it was written.
pub(crate) async fn apply_server_row(tx: &mut Transaction, row: &Value) -> Result<bool, UserError> {
    let user: User = serde_json::from_value(row.clone())?;
    if events::has_pending(tx, TABLE, user.id).await? {
        return Ok(false);
    }
    upsert_in(tx, &user).await?;
    Ok(true)
}

/// Writes the server's row, or inserts it.
pub(crate) async fn upsert_in(tx: &mut Transaction, user: &User) -> Result<(), UserError> {
    Ok(app_users::replication::upsert(&mut *tx, user).await?)
}

/// A page of the rows the server wrote after the cursor that the user
/// sees, deleted or not.
pub(crate) async fn list_synced_after(
    tx: &mut Transaction,
    user: &User,
    after: &Cursor,
    limit: usize,
) -> Result<Vec<User>, UserError> {
    Ok(app_users::replication::list_synced_after(tx, user.id, after, limit).await?)
}

pub(crate) async fn get_in(tx: &mut Transaction, id: Uuid) -> Result<Option<User>, UserError> {
    Ok(app_users::replication::get(tx, id).await?)
}

pub(crate) async fn list_by_ids(
    tx: &mut Transaction,
    ids: &[Uuid],
) -> Result<Vec<User>, UserError> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    Ok(app_users::replication::list_by_ids(tx, ids).await?)
}

async fn get_live(tx: &mut Transaction, id: Uuid) -> Result<User, UserError> {
    match app_users::get(tx, id).await? {
        Some(user) => Ok(user),
        _ => Err(UserError::NotFound { id }),
    }
}

/// A sign-up's name, from its email before the @: `hello@world.com` is
/// `hello`, or `hello1`, `hello2` and on when another user has it. The
/// person changes it on their profile.
pub(crate) async fn name_from_email_in(
    tx: &mut Transaction,
    email: &str,
    id: Uuid,
) -> Result<String, UserError> {
    let before_at = email.split_once('@').map_or(email, |(before, _)| before);
    let mut number = 0_u32;
    loop {
        let suffix = if number == 0 {
            String::new()
        } else {
            number.to_string()
        };
        let start: String = before_at
            .chars()
            .take(MAX_NAME_LENGTH - suffix.len())
            .collect();
        let name = app_users::checked_name(&format!("{start}{suffix}"))?;
        if !app_users::exists_by_name(&mut *tx, &name, id).await? {
            return Ok(name);
        }
        number += 1;
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::database::{Database, with_each_database};

    fn server(database: Database) -> Deployment {
        Deployment::new(database, Role::Server)
    }

    /// A device as the app opens it, with nobody signed in.
    pub(crate) async fn device() -> Deployment {
        let database = Database::open_sqlite_in_memory()
            .await
            .expect("an in-memory SQLite database");
        let deployment = Deployment::new(database, Role::Device);
        device_settings::upgrade(&deployment)
            .await
            .expect("the settings set up");
        deployment
    }

    /// A device after the user signed in on it with the token: their
    /// row is there and the setting names them, as `accounts` writes.
    pub(crate) async fn device_of(user: &User, token: &str) -> Deployment {
        let deployment = device().await;
        let mut tx = deployment.begin().await.expect("a transaction");
        upsert_in(&mut tx, user).await.expect("the user's row");
        let signed_in = SignedIn {
            user_id: user.id,
            token: Some(token.to_string()),
        };
        device_settings::set(&mut tx, SIGNED_IN, &Some(signed_in))
            .await
            .expect("signed in");
        tx.commit().await.expect("committed");
        deployment
    }

    /// A user as the server would have made them, for a device test
    /// with no server.
    pub(crate) fn alice() -> User {
        let now = now();
        User {
            id: Uuid::now_v7(),
            name: "Alice".to_string(),
            inserted_at: now,
            updated_at: now,
            version: 1,
            last_synced_at: Some(now),
            deleted_at: None,
            deleted_by_id: None,
            avatar_url: None,
        }
    }

    #[tokio::test]
    async fn users_delete_in_rolls_back_profile_and_event_together() {
        with_each_database(|database| async move {
            let deployment = server(database);
            let alice = create(&deployment, "alice@example.com", "Alice")
                .await
                .expect("user");
            let mut tx = deployment.begin().await.expect("transaction");
            let deleted = delete_in(&mut tx, alice.id).await.expect("deleted");
            assert!(deleted.deleted_at.is_some());
            assert_eq!(
                events::count(&mut tx, events::EventStatus::Applied)
                    .await
                    .expect("events"),
                1
            );
            drop(tx);
            assert_eq!(get(&deployment, alice.id).await.expect("restored"), alice);
            let mut tx = deployment.begin().await.expect("transaction");
            assert_eq!(
                events::count(&mut tx, events::EventStatus::Applied)
                    .await
                    .expect("events"),
                0
            );
        })
        .await;
    }

    #[tokio::test]
    async fn users_deleted_profile_is_hidden_but_available_anonymized_to_projections() {
        with_each_database(|database| async move {
            let deployment = server(database);
            let alice = create(&deployment, "alice@example.com", "Alice")
                .await
                .expect("user");
            let mut tx = deployment.begin().await.expect("transaction");
            delete_in(&mut tx, alice.id).await.expect("delete");
            tx.commit().await.expect("commit");
            assert!(matches!(
                get(&deployment, alice.id).await,
                Err(UserError::NotFound { .. })
            ));
            let mut tx = deployment.begin().await.expect("transaction");
            assert!(
                app_users::get(&mut tx, alice.id)
                    .await
                    .expect("ordinary read")
                    .is_none()
            );
            let profiles = list_by_ids(&mut tx, &[alice.id]).await.expect("projection");
            assert_eq!(profiles.len(), 1);
            assert_eq!(profiles[0].name, DELETED_NAME);
            assert!(profiles[0].avatar_url.is_none());
            assert!(profiles[0].deleted_at.is_some());
        })
        .await;
    }

    #[tokio::test]
    async fn users_create_on_the_server_is_version_one_and_found_by_id() {
        with_each_database(|database| async move {
            let deployment = server(database);
            let user = create(&deployment, "  Alice@Example.com ", "  Alice ")
                .await
                .expect("a user");
            assert_eq!(user.name, "Alice", "trimmed");
            assert_eq!(user.version, 1);
            assert_eq!(user.last_synced_at, Some(user.updated_at));
            assert_eq!(get(&deployment, user.id).await.expect("the user"), user);

            let account = crate::accounts::users::get(&deployment, user.id)
                .await
                .expect("their account");
            assert_eq!(account.email, "alice@example.com", "trimmed and lowercased");
            assert_eq!(account.confirmed_at, Some(account.inserted_at));

            let error = get(&deployment, Uuid::now_v7()).await.expect_err("missing");
            assert!(matches!(error, UserError::NotFound { .. }));
        })
        .await;
    }

    #[tokio::test]
    async fn users_create_refuses_an_invalid_or_taken_email_and_an_invalid_name() {
        with_each_database(|database| async move {
            let deployment = server(database);
            for email in [" ", "alice", "@example.com", "alice@", "al ice@example.com"] {
                let error = create(&deployment, email, "Alice")
                    .await
                    .expect_err("invalid");
                assert!(
                    matches!(
                        error,
                        UserError::AccountUser(AccountUserError::InvalidEmail)
                    ),
                    "{email}"
                );
            }
            let long = format!(
                "{}@example.com",
                "a".repeat(accounts_users::MAX_EMAIL_LENGTH)
            );
            let error = create(&deployment, &long, "Alice")
                .await
                .expect_err("too long");
            assert!(matches!(
                error,
                UserError::AccountUser(AccountUserError::InvalidEmail)
            ));
            for name in [" ", &"a".repeat(MAX_NAME_LENGTH + 1)] {
                let error = create(&deployment, "alice@example.com", name)
                    .await
                    .expect_err("invalid");
                assert!(
                    matches!(error, UserError::Validation(UserInvalid::Name)),
                    "{name}"
                );
            }

            create(&deployment, "alice@example.com", "Alice")
                .await
                .expect("a user");
            let error = create(&deployment, "alice@example.com", "Alice")
                .await
                .expect_err("taken");
            assert!(matches!(
                error,
                UserError::AccountUser(AccountUserError::EmailTaken)
            ));
        })
        .await;
    }

    #[tokio::test]
    async fn users_name_from_email_numbers_a_taken_name_in_any_case_and_fits_the_length() {
        with_each_database(|database| async move {
            let deployment = server(database);
            create(&deployment, "a@example.com", "Hello")
                .await
                .expect("a user");
            let hello1 = create(&deployment, "b@example.com", "hello1")
                .await
                .expect("a user");
            let gone = create(&deployment, "c@example.com", "hello2")
                .await
                .expect("a user");
            create(&deployment, "d@example.com", &"x".repeat(MAX_NAME_LENGTH))
                .await
                .expect("a user");
            let mut tx = deployment.begin().await.expect("a transaction");
            delete_in(&mut tx, gone.id).await.expect("deleted");

            let new_id = Uuid::now_v7();
            let name = name_from_email_in(&mut tx, "hello@world.com", new_id)
                .await
                .expect("a name");
            assert_eq!(
                name, "hello2",
                "Hello and hello1 are taken, a deleted hello2 is not"
            );
            let own = name_from_email_in(&mut tx, "hello1@world.com", hello1.id)
                .await
                .expect("a name");
            assert_eq!(own, "hello1", "their own name is not taken from them");

            let long = "x".repeat(MAX_NAME_LENGTH + 10);
            let numbered = name_from_email_in(&mut tx, &format!("{long}@world.com"), new_id)
                .await
                .expect("a name");
            assert_eq!(numbered, format!("{}1", "x".repeat(MAX_NAME_LENGTH - 1)));
        })
        .await;
    }

    #[tokio::test]
    async fn users_create_is_the_servers_alone() {
        let deployment = device().await;
        let error = create(&deployment, "alice@example.com", "Alice")
            .await
            .expect_err("a device");
        assert!(matches!(error, UserError::ServerOnly));
    }

    #[tokio::test]
    async fn users_signed_in_on_a_device_is_nobody_until_a_sign_in() {
        let deployment = device().await;
        let error = signed_in(&deployment).await.expect_err("nobody");
        assert!(matches!(error, UserError::NotSignedIn));

        let alice = alice();
        let deployment = device_of(&alice, "a token").await;
        let (user, token) = signed_in_with_token(&deployment)
            .await
            .expect("the signed-in user");
        assert_eq!(user, alice);
        assert_eq!(token, "a token");
    }
}
