//! The quick picker where there is none: every platform but macOS.

use app_core::app::files::{FileSearch, FileSearchResults};
use tauri::AppHandle;
use uuid::Uuid;

use crate::command_error::CommandError;

const UNAVAILABLE: &str = "The quick picker is not available on this device";

pub fn register(_app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}

pub async fn files_search(
    _app: &AppHandle,
    _search: FileSearch,
) -> Result<FileSearchResults, CommandError> {
    Err(UNAVAILABLE.into())
}

pub async fn paste(_app: &AppHandle, _id: Uuid) -> Result<(), CommandError> {
    Err(UNAVAILABLE.into())
}

pub fn close(_app: &AppHandle) {}

pub fn open_app(_app: &AppHandle) -> Result<(), CommandError> {
    Err(UNAVAILABLE.into())
}

pub fn accessibility_request() -> bool {
    false
}
