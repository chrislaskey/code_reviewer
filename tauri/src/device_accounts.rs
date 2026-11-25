//! The accounts on this device, see `docs/design/device-accounts.md`:
//! each with a folder of its own, `accounts/<id>/`, named by an id made
//! on the device, holding its SQLite database, `app.db`, and the bytes
//! of its files, `files/`, see `docs/design/offline-files-decisions.md`;
//! listed in `device.db`, and at most one active, whose deployment and
//! transport every account command takes from here. Each account is
//! one server's user, see `servers` and `docs/design/multi-server.md`.
//! This is the only code that opens, closes, chooses or deletes an
//! account's folder; `app_core` never learns there is more than one.
//! The app data directory, `DATA_DIR` in dev, holds them all. At start
//! the list is swept of orphans, every database is brought up to date,
//! and the last active account is opened again if it is still signed
//! in. One of the accounts may be a local user, made by Skip with no
//! account on any server, see `app::local_users`: it lists, switches and
//! removes like any other, has no sign-out, and becomes an account's own
//! when the person signs up or in and moves what they made.

pub mod servers;

use std::env;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions};
use tauri::{App, AppHandle, Manager, State};
use thiserror::Error;
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;
use uuid::fmt::Hyphenated;

use app_core::accounts::{self, AccountError, Session};
use app_core::app::local_users::{self, LocalUserError, MergeTargetCommunity, Moved};
use app_core::app::users::{self, UserError};
use app_core::data::app_users::User;
use app_core::data::device_settings::{self, DeviceSettingsError};
use app_core::database::{Database, DatabaseError};
use app_core::deployment::{Deployment, FileStore, Role, now};
use app_core::servers::ServerError;
use app_core::servers::settings::Communities;
use app_core::services::web_server::{Http, Transport};
use app_core::sync::{self, SyncError};

use crate::command_error::CommandError;
use crate::server_urls;
use crate::sync::SyncServer;
use servers::DeviceServer;

static MIGRATOR: Migrator = sqlx::migrate!("./migrations/device");

/// How many accounts a device holds at most.
pub const MAX_ACCOUNTS: i64 = 100;
const LIST_FILE: &str = "device.db";
const ACCOUNTS_DIRECTORY: &str = "accounts";
const DATABASE_FILE: &str = "app.db";
const FILES_DIRECTORY: &str = "files";

/// An account on this device, as the accounts page lists it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DeviceAccount {
    /// Made on the device; names the account's folder.
    pub id: Uuid,
    /// The server the account is on; `None` for a local user.
    pub server_id: Option<Uuid>,
    /// The user's id on the server.
    pub user_id: Uuid,
    pub email: String,
    /// The account's file holds a token: it switches to at once;
    /// otherwise it asks for the password first.
    pub signed_in: bool,
    /// A local user: on this device only, with nothing to sign in to.
    pub local: bool,
    pub active: bool,
    pub last_active_at: DateTime<Utc>,
    /// The host of the account's server, `acme.example.com`; `None` on
    /// the default server, and for a local user.
    pub server_host: Option<String>,
    /// What the account's server calls itself; `None` as `server_host`.
    pub server_display_name: Option<String>,
}

/// What finishing a sign-in in the browser answers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum SignInFinished {
    SignedIn {
        user: User,
    },
    /// The device has a local user, and the account is new to it: the
    /// person is asked whether what the local user made moves into the
    /// account, `local_user_move`, or stays apart,
    /// `local_user_keep_separate`. Nothing was done yet.
    MovePending {
        email: String,
        /// The local user's posts and files.
        items: i64,
    },
}

/// What a removal answers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Removal {
    Removed,
    /// Nothing was done: these changes have not reached the server, or
    /// were not kept by it, and the removal would lose them. The page
    /// asks the person, who removes `losing_changes` or keeps the account.
    ChangesWouldBeLost {
        pending: i64,
        conflicts: i64,
    },
}

#[derive(Debug, Error)]
pub enum DeviceAccountsError {
    #[error("Nobody is signed in")]
    NobodyActive,
    #[error("No account {id} on this device")]
    UnknownAccount { id: Uuid },
    #[error("No server {id} on this device")]
    UnknownServer { id: Uuid },
    #[error("{email} is signed out on this device; sign in first")]
    NotSignedIn { email: String },
    #[error("This device holds {MAX_ACCOUNTS} accounts; remove one first")]
    TooManyAccounts,
    #[error("This device already has an account; skipping is for a device with none")]
    AccountsExist,
    #[error("An account that is only on this device has nothing to sign out of")]
    LocalUserHasNoSignOut,
    #[error("No sign-in is waiting to be moved into; sign in again")]
    NoPendingMove,
    #[error("Could not reach {url}: {reason}")]
    ServerUnreachable { url: String, reason: String },
    #[error("could not keep the server's settings: {0}")]
    Settings(#[from] serde_json::Error),
    #[error(transparent)]
    Server(#[from] ServerError),
    #[error(transparent)]
    LocalUser(#[from] LocalUserError),
    #[error(transparent)]
    Account(#[from] AccountError),
    #[error(transparent)]
    User(#[from] UserError),
    #[error(transparent)]
    Sync(#[from] SyncError),
    #[error(transparent)]
    DeviceSettings(#[from] DeviceSettingsError),
    #[error("could not open the account's database: {0}")]
    Open(#[from] DatabaseError),
    #[error("device.db could not be brought up to date: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("file error: {0}")]
    Io(#[from] std::io::Error),
}

/// The list in `device.db`, the folders under `accounts/`, and the
/// active account's open deployment and transport, if any.
pub struct DeviceAccounts {
    directory: PathBuf,
    list: SqlitePool,
    /// The build's default server, see `server_urls`.
    default_server_url: String,
    active: RwLock<Option<Active>>,
    /// The sign-in waiting on the person's answer to the move; in
    /// memory only, so quitting the app means signing in again.
    pending_move: Mutex<Option<PendingMove>>,
}

struct Active {
    id: Uuid,
    server: DeviceServer,
    deployment: Deployment,
    transport: Http,
}

struct PendingMove {
    server_id: Uuid,
    session: Session,
}

#[derive(sqlx::FromRow)]
struct Row {
    id: Hyphenated,
    server_id: Option<Hyphenated>,
    user_id: Hyphenated,
    email: String,
    signed_in: bool,
    local: bool,
    active: bool,
    last_active_at: DateTime<Utc>,
    server_url: Option<String>,
    server_display_name: Option<String>,
}

impl From<Row> for DeviceAccount {
    fn from(row: Row) -> DeviceAccount {
        DeviceAccount {
            id: row.id.into_uuid(),
            server_id: row.server_id.map(Hyphenated::into_uuid),
            user_id: row.user_id.into_uuid(),
            email: row.email,
            signed_in: row.signed_in,
            local: row.local,
            active: row.active,
            last_active_at: row.last_active_at,
            server_host: row.server_url.as_deref().map(host_of),
            server_display_name: row.server_display_name,
        }
    }
}

/// An origin's host and port, as the pages show it: `https://` dropped.
fn host_of(origin: &str) -> String {
    origin
        .split_once("://")
        .map_or(origin, |(_, host)| host)
        .to_string()
}

/// Opens the device's accounts as the app starts and hands them to the
/// commands as managed state.
pub fn open_and_manage(app: &App) -> Result<(), Box<dyn std::error::Error>> {
    let directory = match env::var_os("DATA_DIR") {
        Some(directory) => PathBuf::from(directory),
        None => app.path().app_data_dir()?,
    };
    let default_server_url = server_urls::sync_server_url(app.handle());
    let accounts =
        tauri::async_runtime::block_on(DeviceAccounts::open(&directory, &default_server_url))?;
    log::info!(
        "accounts in {}, default server {default_server_url}",
        directory.display()
    );
    app.manage(accounts);
    Ok(())
}

impl DeviceAccounts {
    /// Opens `device.db` in the directory, creating both when missing,
    /// sweeps the orphans, brings every account's file up to date and
    /// opens the last active account again if it is signed in.
    pub async fn open(
        directory: &Path,
        default_server_url: &str,
    ) -> Result<DeviceAccounts, DeviceAccountsError> {
        std::fs::create_dir_all(directory.join(ACCOUNTS_DIRECTORY))?;
        let options = SqliteConnectOptions::new()
            .filename(directory.join(LIST_FILE))
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal);
        let list = SqlitePoolOptions::new().connect_with(options).await?;
        MIGRATOR.run(&list).await?;
        let accounts = DeviceAccounts {
            directory: directory.to_path_buf(),
            list,
            default_server_url: default_server_url.to_string(),
            active: RwLock::new(None),
            pending_move: Mutex::new(None),
        };
        accounts.default_server_create().await?;
        accounts.sweep().await?;
        accounts.migrate_every_file().await?;
        accounts.open_last_active().await?;
        Ok(accounts)
    }

    /// Every account, the most recently active first.
    pub async fn list(&self) -> Result<Vec<DeviceAccount>, DeviceAccountsError> {
        let rows: Vec<Row> = sqlx::query_as(
            "SELECT a.id, a.server_id, a.user_id, a.email, a.signed_in, a.local, a.active,
               a.last_active_at, s.url AS server_url, s.display_name AS server_display_name
             FROM device_accounts a LEFT JOIN device_servers s ON s.id = a.server_id
             ORDER BY a.last_active_at DESC, a.email",
        )
        .fetch_all(&self.list)
        .await?;
        Ok(rows.into_iter().map(DeviceAccount::from).collect())
    }

    /// The active account's deployment, for the account commands.
    pub async fn active(&self) -> Result<Deployment, DeviceAccountsError> {
        self.active
            .read()
            .await
            .as_ref()
            .map(|active| active.deployment.clone())
            .ok_or(DeviceAccountsError::NobodyActive)
    }

    /// The active account's deployment and the transport to its server,
    /// read together, so a request never carries one account's token
    /// to another's server.
    pub async fn active_with_transport(&self) -> Result<(Deployment, Http), DeviceAccountsError> {
        self.active
            .read()
            .await
            .as_ref()
            .map(|active| (active.deployment.clone(), active.transport.clone()))
            .ok_or(DeviceAccountsError::NobodyActive)
    }

    /// The active account's id on this device.
    pub async fn active_id(&self) -> Option<Uuid> {
        self.active.read().await.as_ref().map(|active| active.id)
    }

    /// The server whose pages the app opens: the active account's, else
    /// the default server.
    pub async fn active_or_default_server(&self) -> Result<DeviceServer, DeviceAccountsError> {
        let active = self
            .active
            .read()
            .await
            .as_ref()
            .map(|active| active.server.clone());
        match active {
            Some(server) => Ok(server),
            None => self.default_server().await,
        }
    }

    /// Whether the active account, if any, is on the default server,
    /// the only one purchases go to, see `docs/design/multi-server.md`.
    pub async fn active_on_default_server(&self) -> bool {
        self.active
            .read()
            .await
            .as_ref()
            .is_none_or(|active| active.server.url.is_none())
    }

    /// The transport to an account's server, for a removal, which may be
    /// of an account that is not active.
    pub async fn transport_of(&self, id: Uuid) -> Result<Http, DeviceAccountsError> {
        let account = self.get(id).await?;
        self.transport_for(&self.server_of(account.server_id).await?)
    }

    /// Keeps a sign-in the server answered: in the account's own file,
    /// made for it when new, which becomes the active one. Whoever was
    /// active before is closed first, and stays signed in.
    pub async fn keep_signed_in(
        &self,
        server_id: Uuid,
        session: Session,
    ) -> Result<User, DeviceAccountsError> {
        self.deactivate().await?;
        let id = match self.find(server_id, session.user.id).await? {
            Some(account) => account.id,
            None if self.count().await? >= MAX_ACCOUNTS => {
                return Err(DeviceAccountsError::TooManyAccounts);
            }
            None => Uuid::now_v7(),
        };
        // The row first, so a crash before the file leaves a row the
        // sweep removes, never a file nothing points at.
        sqlx::query(
            "INSERT INTO device_accounts (id, server_id, user_id, email, signed_in, last_active_at, active)
             VALUES ($1, $2, $3, $4, 0, $5, 0)
             ON CONFLICT (id) DO UPDATE SET email = excluded.email",
        )
        .bind(id.hyphenated())
        .bind(server_id.hyphenated())
        .bind(session.user.id.hyphenated())
        .bind(&session.email)
        .bind(now())
        .execute(&self.list)
        .await?;
        let deployment = self.open_file(id).await?;
        let user = accounts::keep_signed_in(&deployment, session).await?;
        self.mark(id, true, true).await?;
        self.set_active(id, Some(server_id), deployment).await?;
        Ok(user)
    }

    /// Makes a signed-in account the active one, closing whoever was.
    pub async fn activate(&self, id: Uuid) -> Result<User, DeviceAccountsError> {
        let account = self.get(id).await?;
        if !account.signed_in {
            return Err(DeviceAccountsError::NotSignedIn {
                email: account.email,
            });
        }
        self.deactivate().await?;
        let deployment = self.open_file(id).await?;
        let user = users::signed_in(&deployment).await?;
        self.mark(id, true, true).await?;
        self.set_active(id, account.server_id, deployment).await?;
        Ok(user)
    }

    /// Makes the next signed-in account the active one, see
    /// `next_signed_in`: `None`, with nothing done, when there is no other.
    pub async fn activate_next(&self) -> Result<Option<User>, DeviceAccountsError> {
        let Some(active_id) = self.active_id().await else {
            return Ok(None);
        };
        let accounts = self.list().await?;
        match next_signed_in(&accounts, active_id) {
            Some(next_id) => Ok(Some(self.activate(next_id).await?)),
            None => Ok(None),
        }
    }

    /// Closes the active account, if any, which stays signed in. The
    /// commands hold the sync lock around this, so no sync is mid-way.
    pub async fn deactivate(&self) -> Result<(), DeviceAccountsError> {
        if let Some(active) = self.active.write().await.take() {
            sqlx::query("UPDATE device_accounts SET active = 0 WHERE id = $1")
                .bind(active.id.hyphenated())
                .execute(&self.list)
                .await?;
            active.deployment.database.close().await;
        }
        Ok(())
    }

    /// Signs the active account out: the server forgets the token, the
    /// file keeps its data, and nobody is active. Works offline too,
    /// see `accounts::sign_out_on_device`. Not for a local user, who
    /// could never sign back in.
    pub async fn sign_out<T: Transport>(&self, transport: &T) -> Result<(), DeviceAccountsError> {
        let (id, deployment) = self.active_pair().await?;
        if self.get(id).await?.local {
            return Err(DeviceAccountsError::LocalUserHasNoSignOut);
        }
        accounts::sign_out_on_device(&deployment, transport).await?;
        self.mark(id, false, false).await?;
        self.deactivate().await
    }

    /// Signs the active account out without telling the server, which
    /// answered 401: it has forgotten the token already. The data stays.
    pub async fn forget_signed_in(&self) -> Result<(), DeviceAccountsError> {
        let (id, deployment) = self.active_pair().await?;
        accounts::forget_signed_in(&deployment).await?;
        self.mark(id, false, false).await?;
        self.deactivate().await
    }

    /// Removes an account from the device: signs it out, through the
    /// transport to its server, when it is signed in, offline too, then
    /// deletes its folder, the database and the files' bytes, and its
    /// row. Answers
    /// the counts instead while a change would be lost, unless
    /// `losing_changes`.
    pub async fn remove<T: Transport>(
        &self,
        transport: &T,
        id: Uuid,
        losing_changes: bool,
    ) -> Result<Removal, DeviceAccountsError> {
        let account = self.get(id).await?;
        let was_active = self.active_id().await == Some(id);
        let deployment = if was_active {
            self.active().await?
        } else {
            self.open_file(id).await?
        };
        let outcome = self
            .remove_opened(transport, &account, &deployment, losing_changes)
            .await;
        let removed = outcome
            .as_ref()
            .is_ok_and(|outcome| *outcome == Removal::Removed);
        if !was_active {
            deployment.database.close().await;
        } else if removed {
            self.deactivate().await?;
        }
        if removed {
            sqlx::query("DELETE FROM device_accounts WHERE id = $1")
                .bind(id.hyphenated())
                .execute(&self.list)
                .await?;
            delete_folder(&self.folder_of(id))?;
        }
        outcome
    }

    async fn remove_opened<T: Transport>(
        &self,
        transport: &T,
        account: &DeviceAccount,
        deployment: &Deployment,
        losing_changes: bool,
    ) -> Result<Removal, DeviceAccountsError> {
        if !losing_changes {
            let status = sync::status(deployment).await?;
            if status.pending > 0 || status.conflicts > 0 {
                return Ok(Removal::ChangesWouldBeLost {
                    pending: status.pending,
                    conflicts: status.conflicts,
                });
            }
        }
        if account.signed_in && !account.local {
            match accounts::sign_out_on_device(deployment, transport).await {
                Ok(()) | Err(AccountError::User(UserError::NotSignedIn)) => {}
                Err(error) => return Err(error.into()),
            }
            self.mark(account.id, false, false).await?;
        }
        Ok(Removal::Removed)
    }

    /// The device's local user, if it has one.
    pub async fn local_account(&self) -> Result<Option<DeviceAccount>, DeviceAccountsError> {
        Ok(self.list().await?.into_iter().find(|account| account.local))
    }

    /// Whether Skip is offered: the app allows local users and the
    /// device has no account of any kind, so it is the way in for
    /// someone new and is gone once they are in, and after a move.
    pub async fn local_user_can_create(&self) -> Result<bool, DeviceAccountsError> {
        Ok(local_users::ALLOWED && self.count().await? == 0)
    }

    /// Skip: makes the device's local user, with a folder of their own
    /// and a community named for the device, and makes them active. No
    /// request is made.
    pub async fn local_user_create(&self, device_name: &str) -> Result<User, DeviceAccountsError> {
        if !local_users::ALLOWED {
            return Err(LocalUserError::NotAllowed.into());
        }
        if self.count().await? > 0 {
            return Err(DeviceAccountsError::AccountsExist);
        }
        let id = Uuid::now_v7();
        let user_id = Uuid::now_v7();
        let name = local_users::new_name();
        // The row first, as for a sign-in
        sqlx::query(
            "INSERT INTO device_accounts (id, user_id, email, signed_in, local, last_active_at, active)
             VALUES ($1, $2, $3, 0, 1, $4, 0)",
        )
        .bind(id.hyphenated())
        .bind(user_id.hyphenated())
        .bind(local_users::email_of(&name))
        .bind(now())
        .execute(&self.list)
        .await?;
        let deployment = self.open_file(id).await?;
        let user = local_users::create(&deployment, user_id, &name, device_name).await?;
        self.mark(id, true, true).await?;
        self.set_active(id, None, deployment).await?;
        Ok(user)
    }

    /// What a sign-in the browser finished on a server does on this
    /// device: kept as an account of its own, as ever, unless the device
    /// has a local user and the account is new to it. Then nothing is
    /// done, and the session waits for the person's answer.
    pub async fn sign_in_finished(
        &self,
        server_id: Uuid,
        session: Session,
    ) -> Result<SignInFinished, DeviceAccountsError> {
        let local = self.local_account().await?;
        let known = self.find(server_id, session.user.id).await?.is_some();
        let Some(local) = local.filter(|_| !known) else {
            let user = self.keep_signed_in(server_id, session).await?;
            return Ok(SignInFinished::SignedIn { user });
        };
        let items = match self.active_id().await == Some(local.id) {
            true => local_users::count_items(&self.active().await?).await?,
            false => {
                let deployment = self.open_file(local.id).await?;
                let items = local_users::count_items(&deployment).await;
                deployment.database.close().await;
                items?
            }
        };
        let email = session.email.clone();
        *self.pending_move.lock().await = Some(PendingMove { server_id, session });
        Ok(SignInFinished::MovePending { email, items })
    }

    /// The person chose to move: everything the local user made becomes
    /// the account's, which takes the local user's place in the list
    /// and is active. The next sync pushes it. With a merge target, from
    /// `local_user_merge_target_community`, the local user's communities
    /// are merged into it.
    pub async fn local_user_move(
        &self,
        merge_target: Option<MergeTargetCommunity>,
    ) -> Result<User, DeviceAccountsError> {
        let local = self
            .local_account()
            .await?
            .ok_or(LocalUserError::NotLocal)?;
        let PendingMove { server_id, session } = self
            .pending_move
            .lock()
            .await
            .take()
            .ok_or(DeviceAccountsError::NoPendingMove)?;
        if self.active_id().await != Some(local.id) {
            self.activate(local.id).await?;
        }
        // The server first, so a move cut short knows whose account it was
        sqlx::query("UPDATE device_accounts SET server_id = $2 WHERE id = $1")
            .bind(local.id.hyphenated())
            .bind(server_id.hyphenated())
            .execute(&self.list)
            .await?;
        let deployment = self.active().await?;
        let user = local_users::move_to_account(&deployment, session, merge_target).await?;
        self.name_moved_account(local.id, &deployment).await?;
        self.mark(local.id, true, true).await?;
        self.set_active(local.id, Some(server_id), deployment)
            .await?;
        Ok(user)
    }

    /// The account's community to merge the local user's communities
    /// into: on a server with a single community, by its current
    /// settings, the one the account has been a member of the longest;
    /// none on a server with multiple communities, or for an account in
    /// none. Asks the server, which the sign-in just connected to, and
    /// leaves the pending move in place, so a failed request can be
    /// tried again.
    pub async fn local_user_merge_target_community(
        &self,
    ) -> Result<Option<MergeTargetCommunity>, DeviceAccountsError> {
        let (server_id, token) = match self.pending_move.lock().await.as_ref() {
            Some(pending) => (pending.server_id, pending.session.token.clone()),
            None => return Err(DeviceAccountsError::NoPendingMove),
        };
        let transport = self.transport_for(&self.server_of(Some(server_id)).await?)?;
        let settings = transport.servers_current_settings_get().await?.settings;
        if settings.communities != Communities::Single {
            return Ok(None);
        }
        let listed = transport.communities_list(&token).await?;
        Ok(MergeTargetCommunity::oldest_membership(listed))
    }

    /// The person chose to keep them apart: the account is added beside
    /// the local user, which stays as it was.
    pub async fn local_user_keep_separate(&self) -> Result<User, DeviceAccountsError> {
        let PendingMove { server_id, session } = self
            .pending_move
            .lock()
            .await
            .take()
            .ok_or(DeviceAccountsError::NoPendingMove)?;
        self.keep_signed_in(server_id, session).await
    }

    /// Forgets a pending move the person did not answer.
    pub async fn pending_move_forget(&self) {
        *self.pending_move.lock().await = None;
    }

    /// Makes the account with the open deployment the active one, with
    /// the transport to its server.
    async fn set_active(
        &self,
        id: Uuid,
        server_id: Option<Uuid>,
        deployment: Deployment,
    ) -> Result<(), DeviceAccountsError> {
        let server = self.server_of(server_id).await?;
        let transport = self.transport_for(&server)?;
        *self.active.write().await = Some(Active {
            id,
            server,
            deployment,
            transport,
        });
        Ok(())
    }

    async fn active_pair(&self) -> Result<(Uuid, Deployment), DeviceAccountsError> {
        self.active
            .read()
            .await
            .as_ref()
            .map(|active| (active.id, active.deployment.clone()))
            .ok_or(DeviceAccountsError::NobodyActive)
    }

    async fn get(&self, id: Uuid) -> Result<DeviceAccount, DeviceAccountsError> {
        let row: Option<Row> = sqlx::query_as(
            "SELECT a.id, a.server_id, a.user_id, a.email, a.signed_in, a.local, a.active,
               a.last_active_at, s.url AS server_url, s.display_name AS server_display_name
             FROM device_accounts a LEFT JOIN device_servers s ON s.id = a.server_id
             WHERE a.id = $1",
        )
        .bind(id.hyphenated())
        .fetch_optional(&self.list)
        .await?;
        row.map(DeviceAccount::from)
            .ok_or(DeviceAccountsError::UnknownAccount { id })
    }

    /// The account of a server's user, if it is on this device.
    async fn find(
        &self,
        server_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<DeviceAccount>, DeviceAccountsError> {
        let row: Option<Row> = sqlx::query_as(
            "SELECT a.id, a.server_id, a.user_id, a.email, a.signed_in, a.local, a.active,
               a.last_active_at, s.url AS server_url, s.display_name AS server_display_name
             FROM device_accounts a LEFT JOIN device_servers s ON s.id = a.server_id
             WHERE a.server_id = $1 AND a.user_id = $2",
        )
        .bind(server_id.hyphenated())
        .bind(user_id.hyphenated())
        .fetch_optional(&self.list)
        .await?;
        Ok(row.map(DeviceAccount::from))
    }

    async fn count(&self) -> Result<i64, DeviceAccountsError> {
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM device_accounts")
            .fetch_one(&self.list)
            .await?;
        Ok(count)
    }

    async fn mark(
        &self,
        id: Uuid,
        signed_in: bool,
        active: bool,
    ) -> Result<(), DeviceAccountsError> {
        sqlx::query(
            "UPDATE device_accounts SET signed_in = $2, active = $3, last_active_at = $4
             WHERE id = $1",
        )
        .bind(id.hyphenated())
        .bind(signed_in)
        .bind(active)
        .bind(now())
        .execute(&self.list)
        .await?;
        Ok(())
    }

    /// The account's folder, its database and its files' bytes.
    fn folder_of(&self, id: Uuid) -> PathBuf {
        self.directory
            .join(ACCOUNTS_DIRECTORY)
            .join(id.hyphenated().to_string())
    }

    /// Opens an account's database as a device, bringing it and its
    /// settings up to date, with the folder its files' bytes are in.
    async fn open_file(&self, id: Uuid) -> Result<Deployment, DeviceAccountsError> {
        let folder = self.folder_of(id);
        let files = folder.join(FILES_DIRECTORY);
        std::fs::create_dir_all(&files)?;
        let database = Database::open_sqlite_file(&folder.join(DATABASE_FILE)).await?;
        let deployment =
            Deployment::new(database, Role::Device).with_file_store(FileStore::Folder(files));
        device_settings::upgrade(&deployment).await?;
        Ok(deployment)
    }

    /// Deletes a row whose database is missing, a half-finished sign-in,
    /// and whatever under `accounts/` no row names, a half-finished
    /// removal.
    async fn sweep(&self) -> Result<(), DeviceAccountsError> {
        let accounts_directory = self.directory.join(ACCOUNTS_DIRECTORY);
        let mut named = Vec::new();
        for account in self.list().await? {
            if self.folder_of(account.id).join(DATABASE_FILE).exists() {
                named.push(account.id.hyphenated().to_string());
            } else {
                log::warn!(
                    "account {} has no file; removed from the list",
                    account.email
                );
                sqlx::query("DELETE FROM device_accounts WHERE id = $1")
                    .bind(account.id.hyphenated())
                    .execute(&self.list)
                    .await?;
            }
        }
        for entry in std::fs::read_dir(&accounts_directory)? {
            let path = entry?.path();
            let name = path.file_name().and_then(|name| name.to_str());
            if name.is_some_and(|name| !named.iter().any(|folder| folder == name)) {
                log::warn!("{} is no account's; deleted", path.display());
                delete_folder(&path)?;
            }
        }
        Ok(())
    }

    /// Opens and closes every account's file, so a migration that fails
    /// does so now, on the update, and not on a switch weeks later. A
    /// local user whose move was cut short after its file changed has
    /// its row brought up to date here.
    async fn migrate_every_file(&self) -> Result<(), DeviceAccountsError> {
        for account in self.list().await? {
            let deployment = self.open_file(account.id).await?;
            let moved = match account.local {
                true => self.name_moved_account(account.id, &deployment).await,
                false => Ok(None),
            };
            deployment.database.close().await;
            moved?;
        }
        Ok(())
    }

    /// Makes the local user's row the account's, when the file says its
    /// records moved to one: the account's user id and email, signed in,
    /// on the server named when the move started, in the same folder.
    /// The account's user id, if so.
    async fn name_moved_account(
        &self,
        id: Uuid,
        deployment: &Deployment,
    ) -> Result<Option<Uuid>, DeviceAccountsError> {
        let Some(Moved { user_id, email, .. }) = local_users::moved(deployment).await? else {
            return Ok(None);
        };
        sqlx::query(
            "UPDATE device_accounts SET user_id = $2, email = $3, local = 0, signed_in = 1
             WHERE id = $1",
        )
        .bind(id.hyphenated())
        .bind(user_id.hyphenated())
        .bind(email)
        .execute(&self.list)
        .await?;
        local_users::moved_forget(deployment).await?;
        Ok(Some(user_id))
    }

    async fn open_last_active(&self) -> Result<(), DeviceAccountsError> {
        let last = self
            .list()
            .await?
            .into_iter()
            .find(|account| account.active);
        if let Some(account) = last {
            if account.signed_in {
                self.activate(account.id).await?;
            } else {
                self.mark(account.id, false, false).await?;
            }
        }
        Ok(())
    }
}

/// Deletes an account's folder with everything in it, or a stray file.
fn delete_folder(path: &Path) -> std::io::Result<()> {
    let deleted = if path.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    };
    match deleted {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

/// The signed-in account after the active one, in the order they were
/// added to the device, going back to the first after the last. The ids
/// are UUIDv7, so they sort by when they were made. `None` when no other
/// account is signed in.
fn next_signed_in(accounts: &[DeviceAccount], active_id: Uuid) -> Option<Uuid> {
    let mut signed_in: Vec<Uuid> = accounts
        .iter()
        .filter(|account| account.signed_in || account.id == active_id)
        .map(|account| account.id)
        .collect();
    signed_in.sort();
    let position = signed_in.iter().position(|id| *id == active_id)?;
    let next = signed_in[(position + 1) % signed_in.len()];
    (next != active_id).then_some(next)
}

#[tauri::command]
pub async fn device_accounts_list(
    device: State<'_, DeviceAccounts>,
) -> Result<Vec<DeviceAccount>, CommandError> {
    Ok(device.list().await?)
}

/// Switches to a signed-in account, which then syncs.
#[tauri::command]
pub async fn device_accounts_switch(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
    id: Uuid,
) -> Result<User, CommandError> {
    let user = {
        let _no_sync_meanwhile = server.hold().await;
        device.activate(id).await?
    };
    crate::sync::spawn(app);
    Ok(user)
}

/// Switches to the next signed-in account, in the order they were added
/// to the device, which then syncs: `None` when there is no other.
#[tauri::command]
pub async fn device_accounts_switch_next(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
) -> Result<Option<User>, CommandError> {
    let user = {
        let _no_sync_meanwhile = server.hold().await;
        device.activate_next().await?
    };
    if user.is_some() {
        crate::sync::spawn(app);
    }
    Ok(user)
}

/// Closes the active account, before adding another: nobody is active
/// while the sign-in page shows.
#[tauri::command]
pub async fn device_accounts_deactivate(
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
) -> Result<(), CommandError> {
    let _no_sync_meanwhile = server.hold().await;
    Ok(device.deactivate().await?)
}

/// Removes an account from the device, syncing the active one first so
/// a change still waiting reaches the server rather than being lost.
#[tauri::command]
pub async fn device_accounts_remove(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
    id: Uuid,
    losing_changes: bool,
) -> Result<Removal, CommandError> {
    if device.active_id().await == Some(id) {
        // Reported by the sync itself; what it could not push is
        // answered as changes that would be lost.
        let _ = crate::sync::run(&app).await;
    }
    let _no_sync_meanwhile = server.hold().await;
    let transport = device.transport_of(id).await?;
    Ok(device.remove(&transport, id, losing_changes).await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use app_core::app::posts;
    use app_core::services::mailer::Mailer;
    use app_core::services::mailer::mails;
    use app_core::services::web_server::InProcess;

    async fn server() -> Deployment {
        let database = Database::open_sqlite_in_memory()
            .await
            .expect("an in-memory database");
        Deployment::new(database, Role::Server)
    }

    /// A user signed in on the server, signed up the first time, and
    /// finished with the code the memory mailer caught, as a device: the
    /// session its exchange answered.
    async fn signed_up(server: &Deployment, email: &str) -> Session {
        let mailer = Mailer::memory();
        let sent = accounts::sign_in(
            server,
            &mailer,
            email,
            true,
            app_core::helpers::locales::DEFAULT,
        )
        .await
        .expect("signed in");
        let code = mails::code_in(&mailer.last_sent_to(email).expect("mailed")).expect("a code");
        let (verifier, challenge) = accounts::verifier_and_challenge();
        let matched = accounts::sign_in_with_code(server, &sent.pending, &code)
            .await
            .expect("the code matched");
        let device_code =
            accounts::sign_in_confirm_for_device(server, &matched.pending, &challenge)
                .await
                .expect("finished");
        accounts::sign_in_with_device_code(server, &device_code.code, &verifier)
            .await
            .expect("exchanged")
    }

    /// The default server's URL, never asked: the tests sign in through
    /// `InProcess`.
    pub(super) const SERVER_URL: &str = "http://localhost:4000";

    /// Nothing listens on port 1, so a request there is refused at once.
    const UNREACHABLE_URL: &str = "http://127.0.0.1:1";

    async fn default_server_id(device: &DeviceAccounts) -> Uuid {
        device
            .default_server()
            .await
            .expect("the default server")
            .id
    }

    /// The device's id of the account of a user on the default server,
    /// or of the local user.
    async fn id_of(device: &DeviceAccounts, user_id: Uuid) -> Uuid {
        device
            .list()
            .await
            .expect("listed")
            .into_iter()
            .find(|account| account.user_id == user_id)
            .expect("on the device")
            .id
    }

    async fn active_user_id(device: &DeviceAccounts) -> Option<Uuid> {
        let id = device.active_id().await?;
        Some(device.get(id).await.expect("its row").user_id)
    }

    /// The folder of an account, by its id on the device.
    fn folder_made_for(device: &DeviceAccounts, id: Uuid) -> PathBuf {
        device
            .directory
            .join(ACCOUNTS_DIRECTORY)
            .join(id.hyphenated().to_string())
    }

    fn file_exists(device: &DeviceAccounts, id: Uuid) -> bool {
        folder_made_for(device, id).join(DATABASE_FILE).exists()
    }

    #[tokio::test]
    async fn device_accounts_sign_in_switch_sign_out_and_remove() {
        let directory = tempfile::tempdir().expect("a directory");
        let server = server().await;
        let transport = InProcess { server: &server };
        let device = DeviceAccounts::open(directory.path(), SERVER_URL)
            .await
            .expect("opened");
        assert!(matches!(
            device.active().await,
            Err(DeviceAccountsError::NobodyActive)
        ));

        let alice = device
            .keep_signed_in(
                default_server_id(&device).await,
                signed_up(&server, "alice@example.com").await,
            )
            .await
            .expect("alice kept");
        let alice_account = id_of(&device, alice.id).await;
        let post = posts::create(
            &device.active().await.expect("alice's"),
            &alice,
            "hers",
            &[],
        )
        .await
        .expect("a post");
        sync::run(&device.active().await.expect("alice's"), &transport)
            .await
            .expect("synced");

        let bob = device
            .keep_signed_in(
                default_server_id(&device).await,
                signed_up(&server, "bob@example.com").await,
            )
            .await
            .expect("bob kept, alice closed");
        let bob_account = id_of(&device, bob.id).await;
        assert_eq!(active_user_id(&device).await, Some(bob.id));
        let listed = device.list().await.expect("listed");
        assert_eq!(listed.len(), 2);
        assert!(listed.iter().all(|account| account.signed_in));
        assert_eq!(
            listed
                .iter()
                .find(|account| account.active)
                .map(|a| a.user_id),
            Some(bob.id)
        );

        let again = device.activate(alice_account).await.expect("alice again");
        assert_eq!(again, alice);
        let hers = posts::list(&device.active().await.expect("alice's"), &alice)
            .await
            .expect("her posts");
        assert_eq!(hers.len(), 1, "her file was kept while bob was active");
        assert_eq!(hers[0].post.id, post.id);

        device.sign_out(&transport).await.expect("signed out");
        assert_eq!(active_user_id(&device).await, None);
        let error = device
            .activate(alice_account)
            .await
            .expect_err("signed out");
        assert!(matches!(error, DeviceAccountsError::NotSignedIn { .. }));
        assert!(file_exists(&device, alice_account), "her data stays");

        let removed = device
            .remove(&transport, bob_account, false)
            .await
            .expect("removed");
        assert_eq!(removed, Removal::Removed);
        assert!(!file_exists(&device, bob_account));
        assert!(
            !folder_made_for(&device, bob_account).exists(),
            "his folder went, the files' bytes with it"
        );
        assert!(
            folder_made_for(&device, alice_account)
                .join(FILES_DIRECTORY)
                .is_dir()
        );
        assert_eq!(device.list().await.expect("listed").len(), 1);
        let error = device
            .remove(&transport, bob_account, false)
            .await
            .expect_err("gone");
        assert!(matches!(error, DeviceAccountsError::UnknownAccount { .. }));

        let removed = device
            .remove(&transport, alice_account, false)
            .await
            .expect("a signed-out account removes too");
        assert_eq!(removed, Removal::Removed);
        assert!(device.list().await.expect("listed").is_empty());
    }

    fn listed(id: Uuid, signed_in: bool, active: bool) -> DeviceAccount {
        DeviceAccount {
            id,
            server_id: None,
            user_id: Uuid::now_v7(),
            email: format!("{id}@example.com"),
            signed_in,
            local: false,
            active,
            last_active_at: Utc::now(),
            server_host: None,
            server_display_name: None,
        }
    }

    #[test]
    fn device_accounts_next_signed_in_goes_around_in_the_order_added() {
        let first = Uuid::now_v7();
        let second = Uuid::now_v7();
        let third = Uuid::now_v7();
        let accounts = [
            listed(third, true, false),
            listed(first, true, true),
            listed(second, false, false),
        ];
        assert_eq!(
            next_signed_in(&accounts, first),
            Some(third),
            "the signed-out one is skipped, whatever the list's order"
        );
        assert_eq!(
            next_signed_in(&accounts, third),
            Some(first),
            "back to the first"
        );
        assert_eq!(next_signed_in(&[listed(first, true, true)], first), None);
        assert_eq!(
            next_signed_in(
                &[listed(first, true, true), listed(second, false, false)],
                first
            ),
            None
        );
        assert_eq!(
            next_signed_in(&accounts, Uuid::now_v7()),
            None,
            "not listed"
        );
    }

    #[tokio::test]
    async fn device_accounts_switch_next_goes_around_the_signed_in_accounts() {
        let directory = tempfile::tempdir().expect("a directory");
        let server = server().await;
        let transport = InProcess { server: &server };
        let device = DeviceAccounts::open(directory.path(), SERVER_URL)
            .await
            .expect("opened");
        assert_eq!(device.activate_next().await.expect("nobody active"), None);

        let mut users = Vec::new();
        for email in ["alice@example.com", "bob@example.com", "carol@example.com"] {
            let user = device
                .keep_signed_in(
                    default_server_id(&device).await,
                    signed_up(&server, email).await,
                )
                .await
                .expect("kept");
            users.push(user);
        }
        let [alice, bob, carol] = [&users[0], &users[1], &users[2]];
        assert_eq!(active_user_id(&device).await, Some(carol.id));

        let next = device.activate_next().await.expect("switched");
        assert_eq!(next.as_ref(), Some(alice), "back to the first added");
        assert_eq!(active_user_id(&device).await, Some(alice.id));
        let next = device.activate_next().await.expect("switched");
        assert_eq!(next.as_ref(), Some(bob));

        device.sign_out(&transport).await.expect("bob signed out");
        device
            .activate(id_of(&device, carol.id).await)
            .await
            .expect("carol again");
        let next = device.activate_next().await.expect("switched");
        assert_eq!(next.as_ref(), Some(alice), "bob is skipped");
        let next = device.activate_next().await.expect("switched");
        assert_eq!(next.as_ref(), Some(carol));

        device
            .remove(&transport, id_of(&device, alice.id).await, false)
            .await
            .expect("alice removed");
        assert_eq!(
            device.activate_next().await.expect("nobody else"),
            None,
            "carol is the only one signed in"
        );
        assert_eq!(active_user_id(&device).await, Some(carol.id));
    }

    #[tokio::test]
    async fn device_accounts_skip_makes_a_local_user_once_with_no_sign_out_and_removes_it() {
        let directory = tempfile::tempdir().expect("a directory");
        let server = server().await;
        let transport = InProcess { server: &server };
        let device = DeviceAccounts::open(directory.path(), SERVER_URL)
            .await
            .expect("opened");
        assert!(device.local_user_can_create().await.expect("asked"));

        let local = device
            .local_user_create("Alice's Laptop")
            .await
            .expect("skipped");
        let local_account = id_of(&device, local.id).await;
        assert_eq!(active_user_id(&device).await, Some(local.id));
        let listed = device.list().await.expect("listed");
        assert_eq!(listed.len(), 1);
        assert!(listed[0].local && listed[0].signed_in && listed[0].active);
        assert_eq!(listed[0].email, format!("{}@example.com", local.name));
        assert!(!device.local_user_can_create().await.expect("asked"));
        let error = device.local_user_create("again").await.expect_err("once");
        assert!(matches!(error, DeviceAccountsError::AccountsExist));

        let deployment = device.active().await.expect("the local user's");
        posts::create(&deployment, &local, "only here", &[])
            .await
            .expect("a post");
        let report = sync::run(&deployment, &transport).await.expect("a sync");
        assert!(report.local, "nothing is sent");
        let error = device.sign_out(&transport).await.expect_err("no sign-out");
        assert!(matches!(error, DeviceAccountsError::LocalUserHasNoSignOut));

        drop(deployment);
        drop(device);
        let device = DeviceAccounts::open(directory.path(), SERVER_URL)
            .await
            .expect("opened again");
        assert_eq!(
            active_user_id(&device).await,
            Some(local.id),
            "still there after a restart"
        );

        let removed = device
            .remove(&transport, local_account, false)
            .await
            .expect("removed");
        assert_eq!(removed, Removal::Removed);
        assert!(!folder_made_for(&device, local_account).exists());
        assert!(device.local_user_can_create().await.expect("asked"));
    }

    #[tokio::test]
    async fn device_accounts_a_local_user_moves_into_the_account_that_signs_in() {
        let directory = tempfile::tempdir().expect("a directory");
        let server = server().await;
        let transport = InProcess { server: &server };
        let device = DeviceAccounts::open(directory.path(), SERVER_URL)
            .await
            .expect("opened");
        let local = device.local_user_create("Laptop").await.expect("skipped");
        let local_account = id_of(&device, local.id).await;
        let post = posts::create(
            &device.active().await.expect("the local user's"),
            &local,
            "made before signing up",
            &[],
        )
        .await
        .expect("a post");
        assert!(matches!(
            device.local_user_move(None).await,
            Err(DeviceAccountsError::NoPendingMove)
        ));

        let session = signed_up(&server, "alice@example.com").await;
        let alice_id = session.user.id;
        let finished = device
            .sign_in_finished(default_server_id(&device).await, session)
            .await
            .expect("finished");
        assert_eq!(
            finished,
            SignInFinished::MovePending {
                email: "alice@example.com".to_string(),
                items: 1,
            }
        );
        assert_eq!(
            device.list().await.expect("listed").len(),
            1,
            "nothing was done yet"
        );

        let alice = device.local_user_move(None).await.expect("moved");
        assert_eq!(alice.id, alice_id);
        assert_eq!(active_user_id(&device).await, Some(alice.id));
        let listed = device.list().await.expect("listed");
        assert_eq!(listed.len(), 1, "the account took the local user's place");
        assert_eq!(listed[0].id, local_account, "in the same row");
        assert_eq!(listed[0].user_id, alice.id);
        assert_eq!(listed[0].email, "alice@example.com");
        assert!(!listed[0].local && listed[0].signed_in && listed[0].active);
        assert!(file_exists(&device, local_account), "in the same folder");
        assert!(!device.local_user_can_create().await.expect("asked"));

        let deployment = device.active().await.expect("alice's");
        let report = sync::run(&deployment, &transport).await.expect("a sync");
        assert_eq!(report.pending, 0);
        let on_server = posts::get(&server, &alice, post.id)
            .await
            .expect("on the server");
        assert_eq!(on_server.post.inserted_at, post.inserted_at);

        drop(deployment);
        drop(device);
        let device = DeviceAccounts::open(directory.path(), SERVER_URL)
            .await
            .expect("opened again");
        assert_eq!(active_user_id(&device).await, Some(alice.id));
        assert!(
            file_exists(&device, local_account),
            "the sweep keeps a folder its row names"
        );
        let hers = posts::list(&device.active().await.expect("alice's"), &alice)
            .await
            .expect("her posts");
        assert_eq!(hers.len(), 1);

        let removed = device
            .remove(&transport, local_account, false)
            .await
            .expect("removed");
        assert_eq!(removed, Removal::Removed);
        assert!(!folder_made_for(&device, local_account).exists());
    }

    #[tokio::test]
    async fn device_accounts_a_local_user_kept_separate_stays_beside_the_account() {
        let directory = tempfile::tempdir().expect("a directory");
        let server = server().await;
        let device = DeviceAccounts::open(directory.path(), SERVER_URL)
            .await
            .expect("opened");
        let local = device.local_user_create("Laptop").await.expect("skipped");
        posts::create(
            &device.active().await.expect("the local user's"),
            &local,
            "stays here",
            &[],
        )
        .await
        .expect("a post");

        let session = signed_up(&server, "alice@example.com").await;
        let finished = device
            .sign_in_finished(default_server_id(&device).await, session)
            .await
            .expect("finished");
        assert!(matches!(finished, SignInFinished::MovePending { .. }));
        let alice = device.local_user_keep_separate().await.expect("kept apart");
        assert_eq!(active_user_id(&device).await, Some(alice.id));
        let listed = device.list().await.expect("listed");
        assert_eq!(listed.len(), 2);
        assert!(listed.iter().any(|account| account.local));
        let hers = posts::list(&device.active().await.expect("alice's"), &alice)
            .await
            .expect("her posts");
        assert!(hers.is_empty(), "nothing moved");

        let again = signed_up(&server, "alice@example.com").await;
        let finished = device
            .sign_in_finished(default_server_id(&device).await, again)
            .await
            .expect("finished");
        assert!(
            matches!(finished, SignInFinished::SignedIn { .. }),
            "an account already on the device is not asked to move"
        );

        let back = device
            .activate(id_of(&device, local.id).await)
            .await
            .expect("switched back");
        assert_eq!(back, local);
        let theirs = posts::list(&device.active().await.expect("the local user's"), &local)
            .await
            .expect("their posts");
        assert_eq!(theirs.len(), 1);
    }

    #[tokio::test]
    async fn device_accounts_a_move_cut_short_after_the_file_changed_is_finished_at_start() {
        let directory = tempfile::tempdir().expect("a directory");
        let server = server().await;
        let device = DeviceAccounts::open(directory.path(), SERVER_URL)
            .await
            .expect("opened");
        let local = device.local_user_create("Laptop").await.expect("skipped");
        let local_account = id_of(&device, local.id).await;
        let session = signed_up(&server, "alice@example.com").await;
        let alice_id = session.user.id;
        sqlx::query("UPDATE device_accounts SET server_id = $2 WHERE id = $1")
            .bind(local_account.hyphenated())
            .bind(default_server_id(&device).await.hyphenated())
            .execute(&device.list)
            .await
            .expect("the server named, as a move starts");
        let deployment = device.active().await.expect("the local user's");
        local_users::move_to_account(&deployment, session, None)
            .await
            .expect("the file changed, and the app stopped there");
        drop(deployment);
        drop(device);

        let device = DeviceAccounts::open(directory.path(), SERVER_URL)
            .await
            .expect("opened again");
        let listed = device.list().await.expect("listed");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].user_id, alice_id);
        assert_eq!(listed[0].email, "alice@example.com");
        assert!(!listed[0].local);
        assert_eq!(active_user_id(&device).await, Some(alice_id));
        assert!(file_exists(&device, local_account));
    }

    #[tokio::test]
    async fn device_accounts_remove_asks_before_losing_changes() {
        let directory = tempfile::tempdir().expect("a directory");
        let server = server().await;
        let transport = InProcess { server: &server };
        let device = DeviceAccounts::open(directory.path(), SERVER_URL)
            .await
            .expect("opened");
        let alice = device
            .keep_signed_in(
                default_server_id(&device).await,
                signed_up(&server, "alice@example.com").await,
            )
            .await
            .expect("alice kept");
        let alice_account = id_of(&device, alice.id).await;
        let post = posts::create(
            &device.active().await.expect("alice's"),
            &alice,
            "never synced",
            &[],
        )
        .await
        .expect("a post");

        let asked = device
            .remove(&transport, alice_account, false)
            .await
            .expect("asked");
        assert_eq!(
            asked,
            Removal::ChangesWouldBeLost {
                pending: 1,
                conflicts: 0
            }
        );
        assert_eq!(
            active_user_id(&device).await,
            Some(alice.id),
            "nothing done"
        );

        let removed = device
            .remove(&transport, alice_account, true)
            .await
            .expect("removed");
        assert_eq!(removed, Removal::Removed);
        assert_eq!(active_user_id(&device).await, None);
        assert!(!file_exists(&device, alice_account));
        assert!(
            posts::get(&server, &alice, post.id).await.is_err(),
            "the change never reached the server"
        );
    }

    #[tokio::test]
    async fn device_accounts_open_sweeps_orphans_and_opens_the_last_active_again() {
        let directory = tempfile::tempdir().expect("a directory");
        let server = server().await;
        let (alice_id, alice_account) = {
            let device = DeviceAccounts::open(directory.path(), SERVER_URL)
                .await
                .expect("opened");
            let alice = device
                .keep_signed_in(
                    default_server_id(&device).await,
                    signed_up(&server, "alice@example.com").await,
                )
                .await
                .expect("alice kept");
            device
                .keep_signed_in(
                    default_server_id(&device).await,
                    signed_up(&server, "bob@example.com").await,
                )
                .await
                .expect("bob kept");
            device
                .activate(id_of(&device, alice.id).await)
                .await
                .expect("alice active");
            // A row without a database, and a folder without a row
            sqlx::query(
                "INSERT INTO device_accounts (id, server_id, user_id, email, signed_in, last_active_at)
                 VALUES ($1, $2, $3, 'carol@example.com', 1, $4)",
            )
            .bind(Uuid::now_v7().hyphenated())
            .bind(default_server_id(&device).await.hyphenated())
            .bind(Uuid::now_v7().hyphenated())
            .bind(now())
            .execute(&device.list)
            .await
            .expect("a stray row");
            let stray = directory.path().join("accounts/stray/files");
            std::fs::create_dir_all(&stray).expect("a stray folder");
            std::fs::write(stray.join("a-file"), b"bytes").expect("a stray file");
            device.deactivate().await.expect("closed");
            device
                .activate(id_of(&device, alice.id).await)
                .await
                .expect("alice active again");
            (alice.id, id_of(&device, alice.id).await)
        };

        let device = DeviceAccounts::open(directory.path(), SERVER_URL)
            .await
            .expect("opened again");
        assert_eq!(active_user_id(&device).await, Some(alice_id));
        let listed = device.list().await.expect("listed");
        assert_eq!(listed.len(), 2, "the stray row is gone");
        assert!(!directory.path().join("accounts/stray").exists());
        assert!(file_exists(&device, alice_account));
    }

    #[tokio::test]
    async fn device_accounts_a_401_is_a_sign_out_done_elsewhere() {
        let directory = tempfile::tempdir().expect("a directory");
        let server = server().await;
        let transport = InProcess { server: &server };
        let device = DeviceAccounts::open(directory.path(), SERVER_URL)
            .await
            .expect("opened");
        let session = signed_up(&server, "alice@example.com").await;
        let alice = device
            .keep_signed_in(default_server_id(&device).await, session.clone())
            .await
            .expect("alice kept");
        posts::create(
            &device.active().await.expect("alice's"),
            &alice,
            "kept through it",
            &[],
        )
        .await
        .expect("a post");
        accounts::sign_out(&server, &session.token)
            .await
            .expect("the token deleted elsewhere");

        let error = sync::run(&device.active().await.expect("alice's"), &transport)
            .await
            .expect_err("401");
        assert!(matches!(error, SyncError::Unauthorized));
        device.forget_signed_in().await.expect("forgotten");
        assert_eq!(active_user_id(&device).await, None);
        let listed = device.list().await.expect("listed");
        assert!(!listed[0].signed_in);

        let again = device
            .keep_signed_in(
                default_server_id(&device).await,
                signed_up(&server, "alice@example.com").await,
            )
            .await
            .expect("alice kept again");
        assert_eq!(again, alice);
        assert_eq!(
            device.list().await.expect("listed").len(),
            1,
            "back in the same account"
        );
        let status = sync::status(&device.active().await.expect("alice's"))
            .await
            .expect("status");
        assert_eq!(status.pending, 1, "her change waited through it");
    }

    #[tokio::test]
    async fn device_accounts_remove_an_account_deleted_elsewhere() {
        let directory = tempfile::tempdir().expect("a directory");
        let server = server().await;
        let transport = InProcess { server: &server };
        let device = DeviceAccounts::open(directory.path(), SERVER_URL)
            .await
            .expect("opened");
        let session = signed_up(&server, "alice@example.com").await;
        let alice = device
            .keep_signed_in(default_server_id(&device).await, session.clone())
            .await
            .expect("alice kept");
        posts::create(
            &device.active().await.expect("alice's"),
            &alice,
            "lost with it",
            &[],
        )
        .await
        .expect("a post");
        let mailer = Mailer::memory();
        app_core::accounts::deletions::code::create(
            &server,
            &mailer,
            &session.user,
            app_core::helpers::locales::DEFAULT,
        )
        .await
        .expect("a code");
        let code = mails::code_in(&mailer.last_sent_to("alice@example.com").expect("mailed"))
            .expect("a code in it");
        app_core::accounts::deletions::create(
            &server,
            &mailer,
            &session.user,
            Some(&code),
            app_core::helpers::locales::DEFAULT,
        )
        .await
        .expect("deleted on the web");

        let error = sync::run(&device.active().await.expect("alice's"), &transport)
            .await
            .expect_err("410");
        assert!(matches!(error, SyncError::AccountDeleted));
        let removal = device
            .remove(&transport, id_of(&device, alice.id).await, true)
            .await
            .expect("removed");
        assert_eq!(removal, Removal::Removed);
        assert_eq!(active_user_id(&device).await, None);
        assert!(device.list().await.expect("listed").is_empty());
    }

    #[tokio::test]
    async fn device_accounts_sign_out_and_remove_with_the_server_unreachable() {
        let directory = tempfile::tempdir().expect("a directory");
        let server = server().await;
        let unreachable = Http::new(UNREACHABLE_URL).expect("a transport");
        let device = DeviceAccounts::open(directory.path(), SERVER_URL)
            .await
            .expect("opened");
        let alice = device
            .keep_signed_in(
                default_server_id(&device).await,
                signed_up(&server, "alice@example.com").await,
            )
            .await
            .expect("alice kept");
        posts::create(
            &device.active().await.expect("alice's"),
            &alice,
            "kept through the sign-out",
            &[],
        )
        .await
        .expect("a post");

        device
            .sign_out(&unreachable)
            .await
            .expect("signed out offline");
        assert_eq!(active_user_id(&device).await, None);
        let listed = device.list().await.expect("listed");
        assert!(!listed[0].signed_in);

        let bob = device
            .keep_signed_in(
                default_server_id(&device).await,
                signed_up(&server, "bob@example.com").await,
            )
            .await
            .expect("bob kept");
        let bob_id = id_of(&device, bob.id).await;
        posts::create(
            &device.active().await.expect("bob's"),
            &bob,
            "not pushed",
            &[],
        )
        .await
        .expect("a post");
        let removal = device
            .remove(&unreachable, bob_id, false)
            .await
            .expect("asked");
        assert_eq!(
            removal,
            Removal::ChangesWouldBeLost {
                pending: 1,
                conflicts: 0
            }
        );
        let removal = device
            .remove(&unreachable, bob_id, true)
            .await
            .expect("removed offline");
        assert_eq!(removal, Removal::Removed);
        assert!(!file_exists(&device, bob_id));
        assert_eq!(device.list().await.expect("listed").len(), 1);
    }

    #[tokio::test]
    async fn device_accounts_one_user_on_two_servers_is_two_accounts() {
        let directory = tempfile::tempdir().expect("a directory");
        let server = server().await;
        let device = DeviceAccounts::open(directory.path(), SERVER_URL)
            .await
            .expect("opened");
        let default = device.default_server().await.expect("the default");
        assert_eq!(default.url, None, "found at runtime, never stored");
        let other = DeviceServer {
            id: Uuid::now_v7(),
            url: Some("https://staging.example.com".to_string()),
            display_name: None,
            settings: Default::default(),
            settings_version: None,
        };
        sqlx::query("INSERT INTO device_servers (id, url, listed) VALUES ($1, $2, 'unlisted')")
            .bind(other.id.hyphenated())
            .bind(&other.url)
            .execute(&device.list)
            .await
            .expect("a server made from the same database");

        let session = signed_up(&server, "alice@example.com").await;
        let alice = device
            .keep_signed_in(default.id, session.clone())
            .await
            .expect("on the default server");
        assert_eq!(
            device.active_or_default_server().await.expect("a server"),
            default
        );
        device
            .keep_signed_in(other.id, session)
            .await
            .expect("the same user on the other server");
        assert_eq!(
            device.active_or_default_server().await.expect("a server"),
            other,
            "the pages follow the active account's server"
        );

        let listed = device.list().await.expect("listed");
        assert_eq!(listed.len(), 2);
        assert!(listed.iter().all(|account| account.user_id == alice.id));
        assert_ne!(listed[0].id, listed[1].id);
        assert!(
            listed
                .iter()
                .all(|account| file_exists(&device, account.id))
        );

        device.deactivate().await.expect("closed");
        assert_eq!(
            device.active_or_default_server().await.expect("a server"),
            default,
            "with nobody active, the default server's"
        );
    }

    #[tokio::test]
    async fn device_accounts_a_kept_server_names_its_accounts_and_keeps_purchases_off() {
        use app_core::app::features::Features;
        use app_core::servers::settings::{ServerSettings, SettingsDocument};
        use servers::{Listing, ServerChecked};
        let closed = ServerSettings {
            features: Features {
                accounts_sign_up: false,
                ..Features::default()
            },
            ..ServerSettings::default()
        };
        let directory = tempfile::tempdir().expect("a directory");
        let server = server().await;
        let device = DeviceAccounts::open(directory.path(), SERVER_URL)
            .await
            .expect("opened");
        let checked = ServerChecked {
            url: "https://acme.example.com:8443".to_string(),
            host: "acme.example.com:8443".to_string(),
            name: "acme".to_string(),
            display_name: "Acme".to_string(),
            app_identifier: app_core::app::identity::APP_IDENTIFIER.to_string(),
            protocol_version: sync::PROTOCOL_VERSION,
            listing: Listing::Unlisted,
            settings: SettingsDocument::of(closed.clone()),
        };
        let fresh = device.default_server().await.expect("the default");
        assert_eq!(
            (fresh.settings, fresh.settings_version),
            (ServerSettings::default(), None),
            "the build's defaults until first fetched"
        );
        let acme = device.server_keep(&checked).await.expect("kept");
        assert_eq!(acme.url.as_deref(), Some("https://acme.example.com:8443"));
        assert_eq!(acme.settings, closed, "kept with its settings");
        assert_eq!(acme.settings_version, Some(closed.version()));
        let again = device
            .server_keep(&ServerChecked {
                display_name: "Acme Corp".to_string(),
                listing: Listing::Listed,
                ..checked.clone()
            })
            .await
            .expect("kept again");
        assert_eq!(again.id, acme.id, "one row per origin");
        assert_eq!(again.display_name.as_deref(), Some("Acme Corp"));
        let default = device
            .server_keep(&ServerChecked {
                listing: Listing::Default,
                ..checked
            })
            .await
            .expect("the default");
        assert_eq!(default, device.default_server().await.expect("the default"));

        assert!(device.active_on_default_server().await, "nobody active");
        let current = device.current_server().await.expect("current");
        assert!(current.default);
        assert_eq!(
            current.settings, closed,
            "the default server's, kept when it was checked"
        );
        assert_eq!(current.host, "localhost:4000");
        device
            .keep_signed_in(acme.id, signed_up(&server, "alice@example.com").await)
            .await
            .expect("on acme");
        assert!(!device.active_on_default_server().await);
        let current = device.current_server().await.expect("current");
        assert_eq!(
            (
                current.display_name.as_str(),
                current.host.as_str(),
                current.default
            ),
            ("Acme Corp", "acme.example.com:8443", false)
        );
        assert_eq!(current.settings, closed);
        let opened = SettingsDocument::of(ServerSettings::default());
        device
            .server_settings_keep(acme.id, &opened)
            .await
            .expect("kept");
        let current = device.current_server().await.expect("current");
        assert_eq!(
            current.settings,
            ServerSettings::default(),
            "the active account's server's settings, kept again"
        );
        assert_eq!(
            device
                .server_get(acme.id)
                .await
                .expect("acme")
                .settings_version,
            Some(opened.settings_version)
        );
        let on_acme = device.list().await.expect("listed");
        assert_eq!(
            on_acme[0].server_host.as_deref(),
            Some("acme.example.com:8443")
        );
        assert_eq!(on_acme[0].server_display_name.as_deref(), Some("Acme Corp"));

        device
            .keep_signed_in(default.id, signed_up(&server, "bob@example.com").await)
            .await
            .expect("on the default server");
        assert!(device.active_on_default_server().await);
        let bob = device
            .list()
            .await
            .expect("listed")
            .into_iter()
            .find(|account| account.email == "bob@example.com")
            .expect("bob");
        assert_eq!(bob.server_host, None, "the default server shows no host");
    }

    #[tokio::test]
    async fn device_accounts_hold_at_most_the_cap() {
        let directory = tempfile::tempdir().expect("a directory");
        let server = server().await;
        let device = DeviceAccounts::open(directory.path(), SERVER_URL)
            .await
            .expect("opened");
        for n in 0..MAX_ACCOUNTS {
            sqlx::query(
                "INSERT INTO device_accounts (id, server_id, user_id, email, signed_in, last_active_at)
                 VALUES ($1, $2, $3, $4, 0, $5)",
            )
            .bind(Uuid::now_v7().hyphenated())
            .bind(default_server_id(&device).await.hyphenated())
            .bind(Uuid::now_v7().hyphenated())
            .bind(format!("user{n}@example.com"))
            .bind(now())
            .execute(&device.list)
            .await
            .expect("a row");
        }
        let error = device
            .keep_signed_in(
                default_server_id(&device).await,
                signed_up(&server, "one-too-many@example.com").await,
            )
            .await
            .expect_err("full");
        assert!(matches!(error, DeviceAccountsError::TooManyAccounts));
    }
}

/// How long a sign-in started in the browser is waited for, the pending
/// sign-in's own validity on the server.
pub const SIGN_IN_WAIT_MINUTES: i64 =
    app_core::data::accounts_tokens::SIGN_IN_CODE_VALIDITY_MINUTES;

/// A sign-in waiting on the browser, as the accounts page shows it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PendingSignIn {
    /// The server it is on, for the sign-in to open there again.
    pub server_id: Uuid,
    pub email: Option<String>,
    pub started_at: DateTime<Utc>,
}

/// A sign-in waiting on the browser, as its code is exchanged: with
/// the server it started on and the verifier kept for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignInWaiting {
    pub server_id: Uuid,
    pub verifier: String,
}

#[derive(sqlx::FromRow)]
struct SignInRow {
    server_id: Hyphenated,
    verifier: String,
    email: Option<String>,
    started_at: DateTime<Utc>,
}

/// The sign-in waiting on the browser, kept in `device.db` so a phone
/// that ended the app meanwhile picks it up again, see
/// `docs/design/accounts-sign-in.md`.
impl DeviceAccounts {
    /// Keeps the server and the verifier of a sign-in about to open in
    /// the browser, in place of any waiting one.
    pub async fn sign_in_start(
        &self,
        server_id: Uuid,
        verifier: &str,
        email: Option<&str>,
    ) -> Result<(), DeviceAccountsError> {
        sqlx::query(
            "INSERT INTO device_sign_in (id, server_id, verifier, email, started_at)
             VALUES (1, $1, $2, $3, $4)
             ON CONFLICT (id) DO UPDATE SET server_id = excluded.server_id,
             verifier = excluded.verifier, email = excluded.email, started_at = excluded.started_at",
        )
        .bind(server_id.hyphenated())
        .bind(verifier)
        .bind(email)
        .bind(now())
        .execute(&self.list)
        .await?;
        Ok(())
    }

    /// The sign-in waiting on the browser, if one started less than
    /// `SIGN_IN_WAIT_MINUTES` ago; a stale one is forgotten.
    pub async fn sign_in_pending(&self) -> Result<Option<PendingSignIn>, DeviceAccountsError> {
        Ok(self.sign_in_row().await?.map(|row| PendingSignIn {
            server_id: row.server_id.into_uuid(),
            email: row.email,
            started_at: row.started_at,
        }))
    }

    /// The server and verifier of the sign-in waiting on the browser,
    /// for the exchange of its code.
    pub async fn sign_in_waiting(&self) -> Result<Option<SignInWaiting>, DeviceAccountsError> {
        Ok(self.sign_in_row().await?.map(|row| SignInWaiting {
            server_id: row.server_id.into_uuid(),
            verifier: row.verifier,
        }))
    }

    /// Forgets the sign-in waiting on the browser: finished or cancelled.
    pub async fn sign_in_clear(&self) -> Result<(), DeviceAccountsError> {
        sqlx::query("DELETE FROM device_sign_in")
            .execute(&self.list)
            .await?;
        Ok(())
    }

    async fn sign_in_row(&self) -> Result<Option<SignInRow>, DeviceAccountsError> {
        let row: Option<SignInRow> = sqlx::query_as(
            "SELECT server_id, verifier, email, started_at FROM device_sign_in WHERE id = 1",
        )
        .fetch_optional(&self.list)
        .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        if row.started_at < now() - chrono::Duration::minutes(SIGN_IN_WAIT_MINUTES) {
            self.sign_in_clear().await?;
            return Ok(None);
        }
        Ok(Some(row))
    }
}

#[cfg(test)]
mod sign_in_tests {
    use super::tests::SERVER_URL;
    use super::*;

    #[tokio::test]
    async fn device_accounts_a_sign_in_waits_on_the_browser_until_finished_cancelled_or_stale() {
        let directory = tempfile::tempdir().expect("a directory");
        let device = DeviceAccounts::open(directory.path(), SERVER_URL)
            .await
            .expect("opened");
        let server_id = device.default_server().await.expect("the default").id;
        assert_eq!(device.sign_in_pending().await.expect("none"), None);
        assert_eq!(device.sign_in_waiting().await.expect("none"), None);

        device
            .sign_in_start(server_id, "the verifier", Some("alice@example.com"))
            .await
            .expect("started");
        let pending = device
            .sign_in_pending()
            .await
            .expect("one")
            .expect("waiting");
        assert_eq!(pending.email.as_deref(), Some("alice@example.com"));
        assert_eq!(
            device.sign_in_waiting().await.expect("one"),
            Some(SignInWaiting {
                server_id,
                verifier: "the verifier".to_string()
            })
        );
        device
            .sign_in_start(server_id, "another verifier", None)
            .await
            .expect("started again, replacing it");
        assert_eq!(
            device
                .sign_in_waiting()
                .await
                .expect("one")
                .map(|waiting| waiting.verifier)
                .as_deref(),
            Some("another verifier")
        );
        device.sign_in_clear().await.expect("cancelled");
        assert_eq!(device.sign_in_pending().await.expect("none"), None);

        device
            .sign_in_start(server_id, "stale", None)
            .await
            .expect("started");
        let long_ago = now() - chrono::Duration::minutes(SIGN_IN_WAIT_MINUTES + 1);
        sqlx::query("UPDATE device_sign_in SET started_at = $1")
            .bind(long_ago)
            .execute(&device.list)
            .await
            .expect("aged");
        assert_eq!(
            device.sign_in_pending().await.expect("none"),
            None,
            "a stale one is forgotten"
        );
        assert_eq!(device.sign_in_waiting().await.expect("none"), None);
    }
}
