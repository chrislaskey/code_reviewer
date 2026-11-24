//! Where the web page PUTs a file's bytes before it creates the file,
//! see `sync::bytes::uploads`.

use crate::app::files::FileError;
use crate::data::app_files::TABLE;
use crate::data::app_users::User;
use crate::deployment::Deployment;
use crate::sync::bytes::{self, Upload, UploadNew};

/// Answers where to PUT a file's bytes, or that the bucket already has
/// them, see `bytes::uploads::create`.
pub async fn create(
    deployment: &Deployment,
    user: &User,
    request: &UploadNew,
) -> Result<Upload, FileError> {
    Ok(bytes::uploads::create(deployment, user, TABLE, request).await?)
}
