//! A command's error as a form shows it, `{error, errors}`, the web's 422
//! body, so a page shows the fields' messages on either. A command with
//! no form answers its error as a plain message.

use serde::Serialize;

use app_core::app::avatars::AvatarError;
use app_core::app::communities::CommunityError;
use app_core::app::files::FileError;
use app_core::helpers::field_errors::FieldErrors;

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

impl From<FileError> for FormError {
    fn from(error: FileError) -> FormError {
        match error {
            FileError::Invalid(errors) => FormError::of_fields(errors),
            other => FormError::from(other.to_string()),
        }
    }
}

impl From<CommunityError> for FormError {
    fn from(error: CommunityError) -> FormError {
        match error {
            CommunityError::Invalid(errors) => FormError::of_fields(errors),
            other => FormError::from(other.to_string()),
        }
    }
}
