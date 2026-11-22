//! Where to PUT a row's bytes, a URL the server signs for one upload,
//! or that the bucket already has them.

use uuid::Uuid;

use crate::data::app_users::User;
use crate::deployment::Deployment;
use crate::helpers::content_type::content_type_or_unknown;
use crate::helpers::field_errors::FieldErrors;
use crate::sync::bytes::{self, BytesError, SyncedBytes, Upload, UploadNew};

/// How long a signed upload URL works.
const UPLOAD_URL_LIFETIME: std::time::Duration = std::time::Duration::from_secs(10 * 60);

/// Answers where to PUT a row's bytes, or that the bucket already has
/// them: on the server, which holds the bucket's keys. The row does not
/// exist yet, so what holds an upload to its creator is the key's
/// prefix: nobody else writes or claims a row before it exists.
pub async fn create(
    deployment: &Deployment,
    user: &User,
    table: &str,
    request: &UploadNew,
) -> Result<Upload, BytesError> {
    if crate::accounts::trial_users::is_trial(deployment, user).await? {
        return Err(BytesError::Rejected(
            "a trial has no files; sign up first".to_string(),
        ));
    }
    let bytes = bytes::of_table(table)?;
    let storage = bytes::bucket(deployment)?;
    let content_type = content_type_or_unknown(&request.content_type);
    let request = UploadNew {
        content_type: content_type.clone(),
        ..request.clone()
    };
    FieldErrors::check(&request)?;
    checked_size(bytes, request.size)?;
    let id = request.id.unwrap_or_else(Uuid::now_v7);
    let key = bytes::storage_key(bytes, user.id, id);
    let checksum_sha256 = bytes::base64_of_hex(&request.sha256)?;
    if bytes::bucket_has(storage, &key, request.size, &checksum_sha256).await? {
        return Ok(Upload {
            id,
            uploaded: true,
            upload_url: None,
            content_type,
            checksum_sha256,
        });
    }
    let upload_url = storage
        .upload_url(
            &key,
            &content_type,
            Some(&checksum_sha256),
            UPLOAD_URL_LIFETIME,
        )
        .await?;
    Ok(Upload {
        id,
        uploaded: false,
        upload_url: Some(upload_url),
        content_type,
        checksum_sha256,
    })
}

/// A size that is not empty and not more than the table takes.
fn checked_size(bytes: &dyn SyncedBytes, size: u64) -> Result<(), BytesError> {
    let max_bytes = bytes.max_bytes();
    let message = if size == 0 {
        "is empty".to_string()
    } else if size > max_bytes {
        format!("is larger than {} MB", max_bytes / (1024 * 1024))
    } else {
        return Ok(());
    };
    Err(BytesError::Invalid(FieldErrors::one("size", message)))
}
