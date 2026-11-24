//! The files a person already has with the same bytes as those they
//! picked, which the new-files page offers in place of creating them
//! again, see `docs/design/files-communities.md`.

use crate::app::files::{self, FileError, FileWithState};
use crate::data::app_files;
use crate::data::app_users::User;
use crate::deployment::Deployment;
use crate::helpers::field_errors::FieldErrors;
use crate::helpers::sha256::is_sha256;

/// The most checksums `list` takes in one call.
pub const MAX_DUPLICATES_CHECKED: usize = 500;

/// The files the user sees whose bytes have any of these SHA-256s,
/// newest first, each with its state, creator and communities: what a
/// page offers in place of creating the same files again. Only rows the
/// user already sees are compared, on the server and on a device,
/// offline as well, so it tells nothing of files elsewhere. At most
/// `MAX_DUPLICATES_CHECKED` checksums at once.
pub async fn list(
    deployment: &Deployment,
    user: &User,
    sha256s: &[String],
) -> Result<Vec<FileWithState>, FileError> {
    if sha256s.len() > MAX_DUPLICATES_CHECKED {
        return Err(FileError::Invalid(FieldErrors::one(
            "sha256s",
            format!("at most {MAX_DUPLICATES_CHECKED} checksums at once"),
        )));
    }
    for sha256 in sha256s {
        is_sha256(sha256, &()).map_err(|error| {
            FileError::Invalid(FieldErrors::one("sha256s", error.message().to_string()))
        })?;
    }
    if sha256s.is_empty() {
        return Ok(Vec::new());
    }
    let mut tx = deployment.begin().await?;
    let files = app_files::list_with_sha256s(&mut tx, user.id, sha256s).await?;
    let listed = files::with_states(&mut tx, deployment.role, user, files).await?;
    tx.commit().await?;
    Ok(listed)
}
