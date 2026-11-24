//! A command's error as the pages receive it: `{error}`, and with
//! `errors` when a form's fields were wrong, the web's 422 body, so a
//! page shows the same messages on either. Every error a command meets
//! converts into it, so a command uses `?`, as the web's handlers do
//! with `ApiError`.

use std::fmt;

use serde::Serialize;

use app_core::accounts::AccountError;
use app_core::accounts::online::OnlineError;
use app_core::accounts::preferences::PreferenceError;
use app_core::app::avatars::AvatarError;
use app_core::app::communities::CommunityError;
use app_core::app::current_community::CurrentCommunityError;
use app_core::app::files::FileError;
use app_core::app::local_users::LocalUserError;
use app_core::app::memberships::MembershipError;
use app_core::app::posts::PostError;
use app_core::app::users::UserError;
use app_core::helpers::field_errors::FieldErrors;
use app_core::sync::SyncError;
use app_core::sync::conflicts::ConflictError;
use app_core::sync::pending_changes::PendingChangeError;

use crate::device_accounts::DeviceAccountsError;

/// The message, and the fields that were wrong, if any.
#[derive(Debug, Serialize)]
pub struct CommandError {
    error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    errors: Option<FieldErrors>,
}

impl CommandError {
    /// The fields that were wrong.
    pub fn of_fields(errors: FieldErrors) -> CommandError {
        CommandError {
            error: "Some fields need another look".to_string(),
            errors: Some(errors),
        }
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.error)
    }
}

impl From<String> for CommandError {
    fn from(error: String) -> CommandError {
        CommandError {
            error,
            errors: None,
        }
    }
}

impl From<&str> for CommandError {
    fn from(error: &str) -> CommandError {
        CommandError::from(error.to_string())
    }
}

/// Errors the pages show as their message alone.
macro_rules! from_message {
    ($($error:ty),* $(,)?) => {
        $(
            impl From<$error> for CommandError {
                fn from(error: $error) -> CommandError {
                    CommandError::from(error.to_string())
                }
            }
        )*
    };
}

from_message!(
    AccountError,
    ConflictError,
    CurrentCommunityError,
    DeviceAccountsError,
    LocalUserError,
    MembershipError,
    OnlineError,
    PendingChangeError,
    PostError,
    PreferenceError,
    SyncError,
    UserError,
    tauri::Error,
    tokio::task::JoinError,
);

impl From<AvatarError> for CommandError {
    fn from(error: AvatarError) -> CommandError {
        match error {
            AvatarError::Invalid(errors) => CommandError::of_fields(errors),
            other => CommandError::from(other.to_string()),
        }
    }
}

impl From<FileError> for CommandError {
    fn from(error: FileError) -> CommandError {
        match error {
            FileError::Invalid(errors) => CommandError::of_fields(errors),
            other => CommandError::from(other.to_string()),
        }
    }
}

impl From<CommunityError> for CommandError {
    fn from(error: CommunityError) -> CommandError {
        match error {
            CommunityError::Invalid(errors) => CommandError::of_fields(errors),
            other => CommandError::from(other.to_string()),
        }
    }
}
