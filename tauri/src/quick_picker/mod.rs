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

use app_core::app::files::{FileFilter, FileList};
use tauri::AppHandle;
use uuid::Uuid;

use crate::command_error::CommandError;

pub use popup::register;

/// A page of the user's images the clipboard takes that pass the
/// filter, newest first, and how many pass in all; an error when nobody
/// is signed in.
#[tauri::command]
pub async fn quick_picker_files_list(
    app: AppHandle,
    filter: FileFilter,
) -> Result<FileList, CommandError> {
    popup::files_list(&app, filter).await
}

/// Copies the file's image, closes the picker and pastes into the app it
/// opened over.
#[tauri::command]
pub async fn quick_picker_paste(app: AppHandle, id: Uuid) -> Result<(), CommandError> {
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
pub fn quick_picker_open_app(app: AppHandle) -> Result<(), CommandError> {
    popup::open_app(&app)
}

/// Asks macOS for Accessibility access when it isn't granted, which
/// opens the system dialog; answers whether it is granted now.
#[tauri::command]
pub fn quick_picker_accessibility_request() -> bool {
    popup::accessibility_request()
}
