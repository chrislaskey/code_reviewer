//! Opening, closing and pasting from the quick picker's panel on macOS:
//! the shortcut, where the panel opens, and the copy and the ⌘V into
//! the app it opened over.

use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use uuid::Uuid;

use app_core::app::files::{self as core_files, FileSearch, FileSearchResults};

use crate::MAIN_WINDOW;
use crate::app::files;
use crate::clipboard;
use crate::device_accounts::DeviceAccounts;
use crate::macos::ScreenRect;
use crate::macos::running_app::{self, RunningApp};
use crate::macos::{accessibility, keyboard};
use crate::user_auth::signed_in;

use super::panel;
use super::placement::{self, Placement, TextPlacement};

const SHORTCUT_LABEL: &str = "⌃⌘T";
const OPENED_EVENT: &str = "quick-picker-opened";
/// The page the app's window shows when the picker opens it.
const ACCOUNTS_PATH: &str = "/accounts";
/// How long opening waits for the caret before opening at the screen
/// position and moving later.
const CARET_WAIT: Duration = Duration::from_millis(150);
/// How long the app in front gets to take the keyboard back, and the
/// person to release Enter, before ⌘V arrives.
const PASTE_DELAY: Duration = Duration::from_millis(200);

#[derive(Default)]
struct QuickPicker {
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    /// The app in front when the picker opened, to paste into.
    previous_app: Option<RunningApp>,
    /// Counts the opens, so a caret found late moves only the picker it
    /// was asked for.
    open_count: u64,
}

/// What the picker's page is told each time it opens.
#[derive(Clone, Debug, Serialize)]
struct Opened {
    /// Without Accessibility access an item is copied, not pasted.
    accessibility_trusted: bool,
}

/// Registers ⌃⌘T and creates the panel, hidden.
pub fn register(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    app.manage(QuickPicker::default());
    app.plugin(tauri_nspanel::init())?;

    let shortcut = Shortcut::new(Some(Modifiers::SUPER | Modifiers::CONTROL), Code::KeyT);
    app.plugin(
        tauri_plugin_global_shortcut::Builder::new()
            .with_handler(move |app, pressed, event| {
                if pressed == &shortcut
                    && event.state() == ShortcutState::Pressed
                    && let Err(error) = toggle(app)
                {
                    log::warn!("could not toggle the quick picker: {error}");
                }
            })
            .build(),
    )?;
    app.global_shortcut().register(shortcut)?;
    log::info!("registered {SHORTCUT_LABEL} for the quick picker");

    panel::create(app)?;
    Ok(())
}

pub async fn files_search(
    app: &AppHandle,
    search: FileSearch,
) -> Result<FileSearchResults, String> {
    let device = app.state::<DeviceAccounts>();
    let (deployment, user) = signed_in(&device).await?;
    let images = FileSearch {
        content_types: clipboard::image_content_types(),
        ..search
    };
    core_files::search(&deployment, &user, &images)
        .await
        .map_err(|error| error.to_string())
}

/// Opens the picker, or closes it when it's open. Main thread only.
fn toggle(app: &AppHandle) -> Result<(), String> {
    if panel::is_open(app) {
        close(app);
        Ok(())
    } else {
        open(app)
    }
}

/// Opens the picker within `CARET_WAIT` whatever the app in front does:
/// when the caret isn't known in time it opens at the screen position
/// and moves once it is. Main thread only.
fn open(app: &AppHandle) -> Result<(), String> {
    // Read before showing anything, so the picker itself isn't in front
    let previous_app = running_app::frontmost();
    let accessibility_trusted = accessibility::is_trusted(false);
    let open_count = {
        let mut state = state(app);
        state.previous_app = previous_app.clone();
        state.open_count += 1;
        state.open_count
    };

    let fallback_screen = placement::pointer_screen(app)?;
    let lookup = placement::find_text(previous_app.as_ref(), accessibility_trusted);
    let (text, caret_pending) = match lookup.recv_timeout(CARET_WAIT) {
        Ok(text) => (text, false),
        Err(RecvTimeoutError::Timeout) => {
            (Err("the app in front hasn't answered yet".into()), true)
        }
        Err(RecvTimeoutError::Disconnected) => (Err("the caret lookup stopped".into()), false),
    };

    place(app, text, &fallback_screen)?;
    let opened = Opened {
        accessibility_trusted,
    };
    if let Err(error) = app.emit_to(panel::LABEL, OPENED_EVENT, opened) {
        log::warn!("could not tell the quick picker's page it opened: {error}");
    }
    panel::show(app)?;

    if caret_pending {
        move_when_caret_found(app.clone(), lookup, open_count, fallback_screen);
    }
    Ok(())
}

/// Main thread only.
pub fn close(app: &AppHandle) {
    panel::hide(app);
}

/// Main thread only.
pub fn open_app(app: &AppHandle) -> Result<(), String> {
    close(app);
    let window = app
        .get_webview_window(MAIN_WINDOW)
        .ok_or("the app's window is gone")?;
    let mut url = window.url().map_err(|error| error.to_string())?;
    if url.path() != ACCOUNTS_PATH {
        url.set_path(ACCOUNTS_PATH);
        url.set_query(None);
        window.navigate(url).map_err(|error| error.to_string())?;
    }
    window.unminimize().map_err(|error| error.to_string())?;
    window.show().map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())
}

pub fn accessibility_request() -> bool {
    accessibility::is_trusted(true)
}

/// Copies the image to the clipboard, closes the picker and presses ⌘V
/// in the app the picker opened over. Without Accessibility access, or
/// when that app cannot be brought back, the image is copied only.
pub async fn paste(app: &AppHandle, id: Uuid) -> Result<(), String> {
    files::copy_to_clipboard(app, id).await?;
    let previous_app = state(app).previous_app.clone();

    let to_close = app.clone();
    app.run_on_main_thread(move || close(&to_close))
        .map_err(|error| error.to_string())?;

    if !accessibility::is_trusted(false) {
        log::info!("the quick picker copied without pasting: no Accessibility access");
        return Ok(());
    }
    if let Err(error) = bring_to_front(app, previous_app).await {
        log::info!("the quick picker copied without pasting: {error}");
        return Ok(());
    }
    tokio::time::sleep(PASTE_DELAY).await;
    keyboard::press_paste_shortcut()
}

/// The non-activating panel normally leaves the previous app in front.
/// When this app is in front instead (⌃⌘T was pressed in its window),
/// asks macOS to put the previous app back.
async fn bring_to_front(app: &AppHandle, previous_app: Option<RunningApp>) -> Result<(), String> {
    run_on_main_thread(app, move || {
        if !running_app::is_this_app_frontmost() {
            return Ok(());
        }
        match previous_app {
            None => Err("this app is in front and no previous app is known".to_string()),
            Some(previous) if running_app::activate(previous.pid) => Ok(()),
            Some(previous) => Err(format!("macOS refused to activate {}", previous.name)),
        }
    })
    .await
}

async fn run_on_main_thread<T: Send + 'static>(
    app: &AppHandle,
    task: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    let (reply, outcome) = tokio::sync::oneshot::channel();
    app.run_on_main_thread(move || {
        // The caller stopped waiting if this fails; nothing to tell
        let _ = reply.send(task());
    })
    .map_err(|error| error.to_string())?;
    outcome
        .await
        .map_err(|_| "the main thread dropped the task".to_string())?
}

/// Moves the panel to the text, else to the screen position.
fn place(app: &AppHandle, text: TextPlacement, fallback_screen: &ScreenRect) -> Result<(), String> {
    let (placement, screen, wanted) = match text {
        Ok((placement, rect)) => {
            let text_screen =
                placement::screen_at(app, tauri::LogicalPosition::new(rect.x, rect.y));
            let screen = text_screen.unwrap_or(*fallback_screen);
            (placement, screen, placement::position_below(&rect))
        }
        Err(error) => {
            log::debug!("the quick picker found no text to open under: {error}");
            (
                Placement::Screen,
                *fallback_screen,
                placement::position_on_screen(fallback_screen),
            )
        }
    };
    let position = placement::keep_on_screen(wanted, &screen);
    log::debug!("the quick picker is placed by {placement:?} at {position:?}");
    panel::set_position(app, position)
}

/// Waits for a late caret and moves the picker to it, if it's still the
/// same open and still showing.
fn move_when_caret_found(
    app: AppHandle,
    lookup: Receiver<TextPlacement>,
    open_count: u64,
    fallback_screen: ScreenRect,
) {
    std::thread::spawn(move || {
        let Ok(text) = lookup.recv() else {
            return;
        };
        if text.is_err() {
            return;
        }
        let handle = app.clone();
        let moved = app.run_on_main_thread(move || {
            if state(&handle).open_count != open_count || !panel::is_open(&handle) {
                return;
            }
            if let Err(error) = place(&handle, text, &fallback_screen) {
                log::warn!("could not move the quick picker: {error}");
            }
        });
        if let Err(error) = moved {
            log::warn!("could not move the quick picker: {error}");
        }
    });
}

fn state(app: &AppHandle) -> MutexGuard<'_, State> {
    app.state::<QuickPicker>()
        .inner()
        .state
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}
