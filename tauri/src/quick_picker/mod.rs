//! The quick picker: ⌃⌘T anywhere on the Mac opens a small window under
//! the text caret, a grid of the signed-in user's images under a search
//! box. Choosing one copies it to the clipboard, closes the
//! window and pastes it where the person was typing. With nobody signed
//! in the window says so and opens the app. macOS only, see `popup`;
//! elsewhere the commands answer that it is not available. See
//! `docs/design/quick-picker.md`.

#[cfg(target_os = "macos")]
mod panel;
#[cfg(target_os = "macos")]
mod placement;
#[cfg(target_os = "macos")]
mod popup;
#[cfg(not(target_os = "macos"))]
#[path = "unavailable.rs"]
mod popup;

use app_core::app::files::{FileSearch, FileSearchResults};
use tauri::AppHandle;
use uuid::Uuid;

pub use popup::register;

/// A page of the user's images the clipboard takes whose names contain
/// the name searched for, newest first, and how many match in all; an
/// error when nobody is signed in.
#[tauri::command]
pub async fn quick_picker_files_search(
    app: AppHandle,
    search: FileSearch,
) -> Result<FileSearchResults, String> {
    popup::files_search(&app, search).await
}

/// Copies the file's image, closes the picker and pastes into the app it
/// opened over.
#[tauri::command]
pub async fn quick_picker_paste(app: AppHandle, id: Uuid) -> Result<(), String> {
    popup::paste(&app, id).await
}

/// Closes the picker without pasting.
#[tauri::command]
pub fn quick_picker_close(app: AppHandle) {
    popup::close(&app);
}

/// Closes the picker and brings the app's window to the front, on its
/// accounts page, where a sign-in starts.
#[tauri::command]
pub fn quick_picker_open_app(app: AppHandle) -> Result<(), String> {
    popup::open_app(&app)
}

/// Asks macOS for Accessibility access when it isn't granted, which
/// opens the system dialog; answers whether it is granted now.
#[tauri::command]
pub fn quick_picker_accessibility_request() -> bool {
    popup::accessibility_request()
}
