//! Syncing the active account's database with the web server, see
//! `docs/design/offline-sync.md`: `app_core::sync::run` over HTTP to the
//! active account's server, see `device_accounts::servers`. One
//! sync runs at a time: when the app starts, when it comes
//! to the foreground, after every write, from the Sync now button, and
//! every half minute. Each one ends with a `synced` event to the pages,
//! carrying the report or the error. A sync that went through also asks
//! for the person's plan when the one kept is stale, see
//! `app_core::accounts::online`.
//! A device working offline, `sync_set_offline`, makes no request until
//! it goes online again, and neither does one with no account active,
//! see `device_accounts`, which holds the same lock while it switches
//! files. A 401 means the server has forgotten the token: the account is
//! signed out on the device, its data kept. A 410 means the account was
//! deleted, on another device or the web: it is removed from the device
//! with its file, and the pages are told, `account_deleted`.
//! A sync is a spawned task, not a queued job, and the only queue is the
//! pending events: see "How a sync is started and run" in the design.
//! One spawned sync waits at most: a write while one waits starts none,
//! since the waiting one sees it. A sync that left files for the next
//! starts the next. A sync whose answers carried another settings
//! version than the server's kept one fetches its settings again in the
//! background, and tells the pages, `servers-current-changed`.

pub mod conflicts;
pub mod pending_changes;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex as SyncMutex, PoisonError};
use std::time::{Duration, Instant};

use serde_json::json;
use tauri::{App, AppHandle, Emitter, Manager, State};
use tokio::sync::{Mutex, MutexGuard};

use app_core::accounts::online;
use app_core::sync::{self, Report, Status, SyncError};

use crate::command_error::CommandError;
use crate::device_accounts::DeviceAccounts;

const INTERVAL: Duration = Duration::from_secs(30);
/// A foreground sync is skipped this soon after the last sync started: a
/// resume and a focus arrive together, and a click back into a desktop
/// window is not news.
const FOREGROUND_GAP: Duration = Duration::from_secs(5);

/// The lock one sync at a time holds, whether a spawned sync is waiting
/// for it, and when the last sync started.
pub struct SyncServer {
    one_at_a_time: Mutex<()>,
    one_waiting: AtomicBool,
    last_started: SyncMutex<Option<Instant>>,
}

impl SyncServer {
    /// Keeps any sync from starting, and waits for one in flight to
    /// finish, while the guard is held: for the moments the active
    /// account's file changes under the commands.
    pub async fn hold(&self) -> MutexGuard<'_, ()> {
        self.one_at_a_time.lock().await
    }
}

pub fn setup(app: &App) -> Result<(), Box<dyn std::error::Error>> {
    app.manage(SyncServer {
        one_at_a_time: Mutex::new(()),
        one_waiting: AtomicBool::new(false),
        last_started: SyncMutex::new(None),
    });
    spawn(app.handle().clone());
    let handle = app.handle().clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(INTERVAL).await;
            // Logged and reported to the pages by `run` itself
            let _ = run(&handle).await;
        }
    });
    Ok(())
}

/// An online call's error for the pages; one answered 410 starts a
/// sync, which removes the deleted account as it does for its own.
pub fn online_error(app: &AppHandle, error: online::OnlineError) -> CommandError {
    if matches!(
        error,
        online::OnlineError::Server(SyncError::AccountDeleted)
    ) {
        spawn(app.clone());
    }
    error.into()
}

/// Starts a sync in the background, unless one started so is still
/// waiting to run.
pub fn spawn(app: AppHandle) {
    if app
        .state::<SyncServer>()
        .one_waiting
        .swap(true, Ordering::SeqCst)
    {
        return;
    }
    tauri::async_runtime::spawn(async move {
        let _ = run_after_waiting(&app, true).await;
    });
}

/// Starts a sync because the app came to the foreground: the window
/// gained focus, or the app resumed on a phone. Skipped when a sync
/// started within `FOREGROUND_GAP`.
pub fn spawn_on_foreground(app: &AppHandle) {
    let recent = app
        .state::<SyncServer>()
        .last_started
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .is_some_and(|started| started.elapsed() < FOREGROUND_GAP);
    if !recent {
        spawn(app.clone());
    }
}

/// One sync of the active account, reported to the pages as a `synced`
/// event; with nobody active, a report saying so.
pub async fn run(app: &AppHandle) -> Result<Report, String> {
    run_after_waiting(app, false).await
}

/// `run`, by a sync `spawn` started when `spawned`, which is no longer
/// waiting once it holds the lock.
async fn run_after_waiting(app: &AppHandle, spawned: bool) -> Result<Report, String> {
    let server = app.state::<SyncServer>();
    let device = app.state::<DeviceAccounts>();
    let _one_at_a_time = server.one_at_a_time.lock().await;
    if spawned {
        server.one_waiting.store(false, Ordering::SeqCst);
    }
    *server
        .last_started
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = Some(Instant::now());
    let active = device.active_with_transport().await;
    let result = match &active {
        Ok((deployment, transport)) => sync::run(deployment, transport).await,
        Err(_) => Ok(Report {
            signed_out: true,
            ..Report::default()
        }),
    };
    if let Err(SyncError::Unauthorized) = &result {
        log::warn!("the server no longer knows the token: signing out, data kept");
        if let Err(error) = device.forget_signed_in().await {
            log::warn!("could not sign out: {error}");
        }
    }
    let account_deleted = matches!(&result, Err(SyncError::AccountDeleted));
    if account_deleted && let (Some(id), Ok((_, transport))) = (device.active_id().await, &active) {
        log::warn!("the account was deleted: removing it from the device");
        if let Err(error) = device.remove(transport, id, true).await {
            log::warn!("could not remove the deleted account: {error}");
        }
    }
    if let (Ok(report), Ok((deployment, transport))) =
        (&result, device.active_with_transport().await)
        && !report.signed_out
        && !report.local
        && !report.offline
        && online::plan_is_stale(&deployment).await.unwrap_or(true)
    {
        crate::accounts::plans::refresh(&deployment, &transport).await;
    }
    if result.as_ref().is_ok_and(Report::more_files_to_send) {
        spawn(app.clone());
    }
    let result = result.map_err(|error| error.to_string());
    match &result {
        Ok(report) => log::info!("synced: {report:?}"),
        Err(error) => log::warn!("sync failed: {error:?}"),
    }
    let event = json!({
        "report": result.as_ref().ok(),
        "error": result.as_ref().err(),
        "account_deleted": account_deleted,
    });
    if let Err(error) = app.emit("synced", event) {
        log::warn!("could not report the sync: {error}");
    }
    spawn_server_settings_refresh(app.clone());
    result
}

/// Fetches the active account's server's settings again in the
/// background when they changed, and tells the pages.
fn spawn_server_settings_refresh(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        match app
            .state::<DeviceAccounts>()
            .server_settings_refresh()
            .await
        {
            Ok(true) => {
                if let Err(error) = app.emit("servers-current-changed", ()) {
                    log::warn!("could not report the server's settings: {error}");
                }
            }
            Ok(false) => {}
            Err(error) => log::warn!("could not fetch the server's settings: {error}"),
        }
    });
}

#[tauri::command]
pub async fn sync_run(app: AppHandle) -> Result<Report, CommandError> {
    Ok(run(&app).await?)
}

/// Works offline, or online again, which syncs at once.
#[tauri::command]
pub async fn sync_set_offline(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    offline: bool,
) -> Result<(), CommandError> {
    let deployment = device.active().await?;
    sync::set_offline(&deployment, offline).await?;
    if !offline {
        spawn(app);
    }
    Ok(())
}

#[tauri::command]
pub async fn sync_status(device: State<'_, DeviceAccounts>) -> Result<Status, CommandError> {
    let deployment = match device.active().await {
        Ok(deployment) => deployment,
        Err(_) => return Ok(Status::default()),
    };
    Ok(sync::status(&deployment).await?)
}
