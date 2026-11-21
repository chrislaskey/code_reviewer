//! The server's two buckets, chosen at start from the environment, on
//! the S3 API at `STORAGE_ENDPOINT` (AWS when unset) in `STORAGE_REGION`,
//! with `STORAGE_ACCESS_KEY_ID` and `STORAGE_SECRET_ACCESS_KEY`:
//!
//! - `STORAGE_BUCKET`, read by anyone at `STORAGE_PUBLIC_URL`, where
//!   pictures go. Unset, the upload routes answer 503.
//! - `STORAGE_PRIVATE_BUCKET`, read through signed URLs only, where
//!   files go. Unset, the files routes answer 503.
//!
//! `bin/storage` runs Garage for dev and sets them.

use std::env;

use app_core::services::storage::{S3Settings, Storage};
use secrecy::SecretString;

/// The bucket anyone reads, for pictures.
pub fn open_public() -> Result<Option<Storage>, String> {
    let Some(bucket) = variable("STORAGE_BUCKET") else {
        tracing::info!("storage: STORAGE_BUCKET is unset, so uploads are off");
        return Ok(None);
    };
    let public_url = required("STORAGE_BUCKET", "STORAGE_PUBLIC_URL")?;
    open("STORAGE_BUCKET", bucket, Some(public_url))
}

/// The bucket read through signed URLs only, for files.
pub fn open_private() -> Result<Option<Storage>, String> {
    let Some(bucket) = variable("STORAGE_PRIVATE_BUCKET") else {
        tracing::info!("storage: STORAGE_PRIVATE_BUCKET is unset, so files are off");
        return Ok(None);
    };
    open("STORAGE_PRIVATE_BUCKET", bucket, None)
}

fn open(
    named_by: &str,
    bucket: String,
    public_url: Option<String>,
) -> Result<Option<Storage>, String> {
    let settings = S3Settings {
        endpoint: variable("STORAGE_ENDPOINT"),
        region: required(named_by, "STORAGE_REGION")?,
        access_key_id: required(named_by, "STORAGE_ACCESS_KEY_ID")?,
        secret_access_key: SecretString::from(required(named_by, "STORAGE_SECRET_ACCESS_KEY")?),
        public_url,
        bucket,
    };
    tracing::info!(
        "storage: bucket {} at {}",
        settings.bucket,
        settings.endpoint.as_deref().unwrap_or("AWS")
    );
    Storage::s3(settings)
        .map(Some)
        .map_err(|error| error.to_string())
}

fn variable(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.is_empty())
}

fn required(named_by: &str, name: &str) -> Result<String, String> {
    variable(name).ok_or_else(|| format!("{named_by} is set, so {name} is needed too"))
}
