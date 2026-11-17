//! JSON files in the data directory that only the user can read, written
//! whole or not at all.

use std::fs;
use std::io::{ErrorKind, Write};
use std::path::Path;

use serde::Serialize;
use serde::de::DeserializeOwned;

/// The value in the JSON file at `path`, or `T::default()` when there is no file.
pub fn read_json_or_default<T: DeserializeOwned + Default>(path: &Path) -> Result<T, String> {
    match fs::read_to_string(path) {
        Ok(text) => {
            serde_json::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(T::default()),
        Err(error) => Err(format!("{}: {error}", path.display())),
    }
}

/// Writes `value` as JSON beside `path`, readable only by the user, and
/// moves it into place, so a crash never leaves half a file.
pub fn write_private_json<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<(), String> {
    let text = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
    let partial_path = path.with_extension("json.partial");
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options
        .open(&partial_path)
        .map_err(|error| format!("{}: {error}", partial_path.display()))?;
    file.write_all(text.as_bytes())
        .map_err(|error| format!("{}: {error}", partial_path.display()))?;
    fs::rename(&partial_path, path).map_err(|error| format!("{}: {error}", path.display()))
}
