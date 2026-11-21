//! The files commands: `app_core::app::files` as the pages call it with
//! `invoke`, for the user signed in on this device, see
//! `docs/design/offline-files-decisions.md`. A form's error is
//! `{error, errors}`, the web's 422 body. A write starts a sync in the
//! background once it is on the device, see `sync`.
//!
//! A page reads a file's bytes through the `files` protocol,
//! `content_response`, which serves the copy in the account's folder,
//! fetched from the server the first time when this device did not add
//! the file, so an `<img>` shows it with the network off from then on.

use percent_encoding::percent_decode_str;
use tauri::http::header::{CONTENT_TYPE, HeaderValue, X_CONTENT_TYPE_OPTIONS};
use tauri::http::{Response, StatusCode, Uri};
use tauri::ipc::{InvokeBody, Request};
use tauri::{AppHandle, Manager, State};
use uuid::Uuid;

use app_core::app::files::{self, FileSearch, FileSearchResults, FileWithState, FilesCache};
use app_core::data::app_files::{self, File};

use crate::command_error::CommandError;
use crate::device_accounts::DeviceAccounts;
use crate::sync::{self, SyncServer};
use crate::user_auth::signed_in;

/// How many names a download tries before giving up.
const FREE_NAME_TRIES: u32 = 1000;
const FILE_NAME_HEADER: &str = "x-file-name";
const FILE_CONTENT_TYPE_HEADER: &str = "x-file-content-type";
const FILE_COMMUNITY_IDS_HEADER: &str = "x-file-community-ids";

/// A page of the files the user sees on this device whose names contain
/// the name searched for, and how many match in all.
#[tauri::command]
pub async fn files_search(
    device: State<'_, DeviceAccounts>,
    search: FileSearch,
) -> Result<FileSearchResults, CommandError> {
    let (deployment, user) = signed_in(&device).await?;
    Ok(files::search(&deployment, &user, &search).await?)
}

/// The files the user sees on this device with any of these checksums.
#[tauri::command]
pub async fn files_list_duplicates(
    device: State<'_, DeviceAccounts>,
    sha256s: Vec<String>,
) -> Result<Vec<FileWithState>, CommandError> {
    let (deployment, user) = signed_in(&device).await?;
    Ok(files::list_duplicates(&deployment, &user, &sha256s).await?)
}

#[tauri::command]
pub async fn files_get(
    device: State<'_, DeviceAccounts>,
    id: Uuid,
) -> Result<FileWithState, CommandError> {
    let (deployment, user) = signed_in(&device).await?;
    Ok(files::get(&deployment, &user, id).await?)
}

/// Creates the file the person picked: its bytes are the command's raw
/// body; its name, percent-encoded, its content type and its
/// communities' ids, joined by commas, are headers. The file is kept on
/// this device and sent by the next sync.
#[tauri::command]
pub async fn files_create(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    request: Request<'_>,
) -> Result<File, CommandError> {
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err(CommandError::from(
            "the file's bytes are the command's body".to_string(),
        ));
    };
    let header = |name: &str| {
        request
            .headers()
            .get(name)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
    };
    let name = percent_decode_str(header(FILE_NAME_HEADER)).decode_utf8_lossy();
    let community_ids = header(FILE_COMMUNITY_IDS_HEADER)
        .split(',')
        .filter(|id| !id.is_empty())
        .map(Uuid::parse_str)
        .collect::<Result<Vec<Uuid>, _>>()
        .map_err(|error| CommandError::from(error.to_string()))?;
    let (deployment, user) = signed_in(&device).await?;
    let file = files::create_with_bytes(
        &deployment,
        &user,
        &name,
        header(FILE_CONTENT_TYPE_HEADER),
        &community_ids,
        bytes,
    )
    .await?;
    sync::spawn(app);
    Ok(file)
}

#[tauri::command]
pub async fn files_update(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    id: Uuid,
    name: String,
    community_ids: Vec<Uuid>,
) -> Result<File, CommandError> {
    let (deployment, user) = signed_in(&device).await?;
    let file = files::update(&deployment, &user, id, &name, &community_ids).await?;
    sync::spawn(app);
    Ok(file)
}

#[tauri::command]
pub async fn files_delete(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    id: Uuid,
) -> Result<(), CommandError> {
    let (deployment, user) = signed_in(&device).await?;
    files::delete(&deployment, &user, id).await?;
    sync::spawn(app);
    Ok(())
}

/// The copies this device keeps of files the server holds too.
#[tauri::command]
pub async fn files_cache_get(
    device: State<'_, DeviceAccounts>,
) -> Result<FilesCache, CommandError> {
    let (deployment, user) = signed_in(&device).await?;
    Ok(files::cache_get(&deployment, &user).await?)
}

/// Removes those copies, and answers what went.
#[tauri::command]
pub async fn files_cache_delete(
    device: State<'_, DeviceAccounts>,
) -> Result<FilesCache, CommandError> {
    let (deployment, user) = signed_in(&device).await?;
    Ok(files::cache_delete(&deployment, &user).await?)
}

/// Whether this build copies an image to the clipboard: macOS and iOS
/// do.
#[tauri::command]
pub fn files_copy_supported() -> bool {
    cfg!(any(target_os = "macos", target_os = "ios"))
}

/// Copies the image to the clipboard, from this device's copy of it,
/// fetched first when there is none.
#[tauri::command]
pub async fn files_copy(app: AppHandle, id: Uuid) -> Result<(), CommandError> {
    copy_to_clipboard(&app, id).await
}

/// `files_copy`, for the quick picker too.
pub async fn copy_to_clipboard(app: &AppHandle, id: Uuid) -> Result<(), CommandError> {
    let device = app.state::<DeviceAccounts>();
    let server = app.state::<SyncServer>();
    let (deployment, user) = signed_in(&device).await?;
    let shown = files::get(&deployment, &user, id).await?;
    let path = files::local_copy(&deployment, server.transport(), &user, id).await?;
    Ok(copy_image(app, &shown.file, path).await?)
}

/// AppKit's and UIKit's pasteboards and images are the main thread's.
#[cfg(any(target_os = "macos", target_os = "ios"))]
async fn copy_image(app: &AppHandle, file: &File, path: std::path::PathBuf) -> Result<(), String> {
    use crate::clipboard::{self, ClipboardImage};

    let uniform_type = clipboard::uniform_type_of(&file.content_type)
        .ok_or("This kind of image cannot be copied yet")?;
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|error| error.to_string())?;
    #[cfg(target_os = "macos")]
    let path = copy_under_its_name(app, &path, &file.name, &file.content_type).await?;
    let (copied, result) = tokio::sync::oneshot::channel();
    app.run_on_main_thread(move || {
        let image = ClipboardImage {
            bytes: &bytes,
            uniform_type,
            file: &path,
        };
        // The command stopped waiting if this fails; nothing to tell
        let _ = copied.send(clipboard::copy_image(image).map(|_types| ()));
    })
    .map_err(|error| error.to_string())?;
    result.await.map_err(|error| error.to_string())?
}

/// A copy of the file under its own name, in the cache's `clipboard`
/// folder, which holds only the last one: an app pasting the file takes
/// its name and its extension from the path, and the account's folder
/// keeps a file under its id alone.
#[cfg(target_os = "macos")]
async fn copy_under_its_name(
    app: &AppHandle,
    from: &std::path::Path,
    name: &str,
    content_type: &str,
) -> Result<std::path::PathBuf, String> {
    let folder = app
        .path()
        .app_cache_dir()
        .map_err(|error| error.to_string())?
        .join("clipboard");
    // Nothing to remove the first time
    let _ = tokio::fs::remove_dir_all(&folder).await;
    tokio::fs::create_dir_all(&folder)
        .await
        .map_err(|error| error.to_string())?;
    let to = folder.join(clipboard_file_name(name, content_type));
    tokio::fs::copy(from, &to)
        .await
        .map_err(|error| error.to_string())?;
    Ok(to)
}

/// The file's name as a file's on the clipboard: only its last part,
/// so it stays in the folder, ending in the extension of its type.
#[cfg(any(target_os = "macos", test))]
fn clipboard_file_name(name: &str, content_type: &str) -> String {
    let name = std::path::Path::new(name)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("image");
    let extensions: &[&str] = match content_type.split(';').next().unwrap_or_default().trim() {
        "image/png" => &["png"],
        "image/jpeg" => &["jpg", "jpeg"],
        "image/gif" => &["gif"],
        "image/webp" => &["webp"],
        "image/tiff" => &["tiff", "tif"],
        "image/bmp" => &["bmp"],
        "image/heic" => &["heic"],
        _ => &[],
    };
    let named = std::path::Path::new(name)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extensions.contains(&extension.to_lowercase().as_str()));
    match extensions.first() {
        Some(extension) if !named => format!("{name}.{extension}"),
        _ => name.to_string(),
    }
}

#[cfg(not(any(target_os = "macos", target_os = "ios")))]
async fn copy_image(
    _app: &AppHandle,
    _file: &File,
    _path: std::path::PathBuf,
) -> Result<(), String> {
    Err("Copying is not available on this device yet".to_string())
}

/// Whether this build saves a copy of a file outside the app: the
/// desktop does, into the Downloads folder; a phone does not yet.
#[tauri::command]
pub fn files_download_supported() -> bool {
    cfg!(desktop)
}

/// Copies the file from this device into the Downloads folder under its
/// name, or the name with a number when that is taken; answers where.
#[tauri::command]
pub async fn files_download(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
    id: Uuid,
) -> Result<String, CommandError> {
    if !cfg!(desktop) {
        return Err("Downloading is not available on this device yet".into());
    }
    let (deployment, user) = signed_in(&device).await?;
    let shown = files::get(&deployment, &user, id).await?;
    let from = files::local_copy(&deployment, server.transport(), &user, id).await?;
    let downloads = app.path().download_dir()?;
    let to = tokio::task::spawn_blocking(move || {
        copy_under_a_free_name(&from, &downloads, &shown.file.name)
    })
    .await?
    .map_err(|error| format!("could not save the file: {error}"))?;
    Ok(to.display().to_string())
}

/// Copies the file into the folder under the name, without replacing
/// anything there: `notes.txt`, then `notes (1).txt`, and so on.
fn copy_under_a_free_name(
    from: &std::path::Path,
    folder: &std::path::Path,
    name: &str,
) -> std::io::Result<std::path::PathBuf> {
    // A name is the person's text: only its last part, so it stays in the folder
    let name = std::path::Path::new(name)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("file");
    let (stem, extension) = match name.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() => (stem, format!(".{extension}")),
        _ => (name, String::new()),
    };
    let mut bytes = std::fs::File::open(from)?;
    for number in 0..FREE_NAME_TRIES {
        let candidate = match number {
            0 => folder.join(name),
            number => folder.join(format!("{stem} ({number}){extension}")),
        };
        match std::fs::File::create_new(&candidate) {
            Ok(mut copy) => {
                std::io::copy(&mut bytes, &mut copy)?;
                return Ok(candidate);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        format!("{name} and its numbered copies are all taken"),
    ))
}

/// Answers a request of the `files` protocol, `files://localhost/<id>`:
/// the bytes of the signed-in user's file from this device, fetched
/// first when they are not here, with the row's content type, or 404. `nosniff`, so the webview shows a file as
/// the type its row says and never as a page of its own guessing.
pub async fn content_response(app: &AppHandle, uri: &Uri) -> Response<Vec<u8>> {
    match content(app, uri).await {
        Some((content_type, bytes)) => {
            let mut response = Response::new(bytes);
            let content_type = HeaderValue::from_str(&content_type)
                .unwrap_or(HeaderValue::from_static(app_files::UNKNOWN_CONTENT_TYPE));
            response.headers_mut().insert(CONTENT_TYPE, content_type);
            response
                .headers_mut()
                .insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
            response
        }
        None => {
            let mut response = Response::new(Vec::new());
            *response.status_mut() = StatusCode::NOT_FOUND;
            response
        }
    }
}

/// The content type and the bytes of the file the URI names, when the
/// signed-in user has it on this device.
async fn content(app: &AppHandle, uri: &Uri) -> Option<(String, Vec<u8>)> {
    let id = Uuid::parse_str(uri.path().trim_start_matches('/')).ok()?;
    let device = app.state::<DeviceAccounts>();
    let (deployment, user) = signed_in(&device).await.ok()?;
    let shown = files::get(&deployment, &user, id).await.ok()?;
    let server = app.state::<SyncServer>();
    let path = files::local_copy(&deployment, server.transport(), &user, id)
        .await
        .inspect_err(|error| log::info!("file {id} is not shown: {error}"))
        .ok()?;
    let bytes = tokio::fs::read(path).await.ok()?;
    Some((shown.file.content_type, bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_clipboard_file_name_ends_in_the_extension_of_its_type() {
        assert_eq!(clipboard_file_name("cat.gif", "image/gif"), "cat.gif");
        assert_eq!(
            clipboard_file_name("Photo.JPEG", "image/jpeg"),
            "Photo.JPEG"
        );
        assert_eq!(clipboard_file_name("cat", "image/webp"), "cat.webp");
        assert_eq!(clipboard_file_name("cat.txt", "image/png"), "cat.txt.png");
        assert_eq!(clipboard_file_name("../up/cat.png", "image/png"), "cat.png");
        assert_eq!(clipboard_file_name("..", "image/png"), "image.png");
    }
}
