//! The quick picker where there is none: every platform but macOS.

use app_core::files::{FileSearch, FileSearchResults};
use tauri::AppHandle;
use uuid::Uuid;

const UNAVAILABLE: &str = "The quick picker is not available on this device";

pub fn register(_app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}

pub async fn files_search(
    _app: &AppHandle,
    _search: FileSearch,
) -> Result<FileSearchResults, String> {
    Err(UNAVAILABLE.to_string())
}

pub async fn paste(_app: &AppHandle, _id: Uuid) -> Result<(), String> {
    Err(UNAVAILABLE.to_string())
}

pub fn close(_app: &AppHandle) {}

pub fn open_app(_app: &AppHandle) -> Result<(), String> {
    Err(UNAVAILABLE.to_string())
}

pub fn accessibility_request() -> bool {
    false
}
