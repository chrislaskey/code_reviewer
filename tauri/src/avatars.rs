//! The avatars commands: `app_core::app::avatars` as the pages call it with
//! `invoke`, for the user signed in on this device. Online only: both
//! ask the web server, which holds the bucket's keys; the page PUTs the
//! file to the URL the first answers. An error is `{error, errors}`, the
//! web's 422 body, so a page shows the fields' messages either way.

use serde::Serialize;
use tauri::State;

use app_core::app::avatars::{self, AvatarError, AvatarUpdate, AvatarUpload, AvatarUploadNew};
use app_core::app::users::User;
use app_core::helpers::field_errors::FieldErrors;

use crate::device_accounts::DeviceAccounts;
use crate::sync::SyncServer;

/// A form's error: the message, and the fields that were wrong, if any.
#[derive(Debug, Serialize)]
pub struct FormError {
    error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    errors: Option<FieldErrors>,
}

impl FormError {
    /// The fields that were wrong.
    pub fn of_fields(errors: FieldErrors) -> FormError {
        FormError {
            error: "Some fields need another look".to_string(),
            errors: Some(errors),
        }
    }
}

impl From<AvatarError> for FormError {
    fn from(error: AvatarError) -> FormError {
        match error {
            AvatarError::Invalid(errors) => FormError::of_fields(errors),
            other => FormError::from(other.to_string()),
        }
    }
}

impl From<String> for FormError {
    fn from(error: String) -> FormError {
        FormError {
            error,
            errors: None,
        }
    }
}

#[tauri::command]
pub async fn avatars_create_upload(
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
    content_type: String,
    size: u64,
) -> Result<AvatarUpload, FormError> {
    let deployment = device.active().await.map_err(|error| error.to_string())?;
    let request = AvatarUploadNew { content_type, size };
    Ok(avatars::create_upload_on_device(&deployment, server.transport(), &request).await?)
}

#[tauri::command]
pub async fn avatars_update(
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
    key: String,
) -> Result<User, FormError> {
    let deployment = device.active().await.map_err(|error| error.to_string())?;
    let request = AvatarUpdate { key };
    Ok(avatars::update_on_device(&deployment, server.transport(), &request).await?)
}
