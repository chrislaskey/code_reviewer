//! Object storage: an S3-style bucket the server hands out signed upload
//! URLs for, so a file goes from the person's device straight to the
//! bucket and never through the server. Server only: a device holds no
//! keys and asks the server, see `avatars`. In dev the bucket is Garage's
//! (`bin/storage`); in tests it is in memory.

use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use chrono::{DateTime, Utc};
use futures_util::TryStreamExt;

use object_store::aws::{AmazonS3, AmazonS3Builder};
use object_store::memory::InMemory;
use object_store::path::Path;
use object_store::signer::{HeaderValue, Method, SignedUrlOptions, Signer};
use object_store::{ObjectStore, ObjectStoreExt, PutPayload};
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// The header a PUT carries the SHA-256 of its bytes in, base64, and the
/// bucket checks them against.
pub const CHECKSUM_SHA256_HEADER: &str = "x-amz-checksum-sha256";
/// How long the URL of the server's own HEAD request works.
const HEAD_URL_LIFETIME: Duration = Duration::from_secs(60);

/// A file in the bucket: its key, and when it was written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredFile {
    pub key: String,
    pub last_modified: DateTime<Utc>,
}

#[derive(Debug, Error)]
#[error("object storage: {0}")]
pub struct StorageError(String);

impl From<object_store::Error> for StorageError {
    fn from(error: object_store::Error) -> StorageError {
        StorageError(error.to_string())
    }
}

/// Where the server's bucket is and the keys it signs with.
pub struct S3Settings {
    /// The S3 API's URL for a store other than AWS, such as Garage or R2.
    pub endpoint: Option<String>,
    pub region: String,
    pub bucket: String,
    pub access_key_id: String,
    pub secret_access_key: SecretString,
    /// Where the bucket's objects are read from by anyone, such as a CDN,
    /// without a trailing slash. None for a private bucket, read through
    /// signed URLs only.
    pub public_url: Option<String>,
}

#[derive(Clone, Debug)]
pub enum Storage {
    S3 {
        store: Arc<AmazonS3>,
        public_url: Option<String>,
        /// For the requests `object_store` does not make: reading a file's
        /// checksum.
        http: reqwest::Client,
    },
    /// For tests: the server's half works, and a test puts the files
    /// where a device's upload would have.
    Memory { store: Arc<InMemory> },
}

impl Storage {
    pub fn s3(settings: S3Settings) -> Result<Storage, StorageError> {
        let mut builder = AmazonS3Builder::new()
            .with_region(settings.region)
            .with_bucket_name(settings.bucket)
            .with_access_key_id(settings.access_key_id)
            .with_secret_access_key(settings.secret_access_key.expose_secret());
        if let Some(endpoint) = settings.endpoint {
            builder = builder
                .with_allow_http(endpoint.starts_with("http://"))
                .with_endpoint(endpoint);
        }
        Ok(Storage::S3 {
            store: Arc::new(builder.build()?),
            http: reqwest::Client::new(),
            public_url: settings
                .public_url
                .map(|url| url.trim_end_matches('/').to_string()),
        })
    }

    pub fn memory() -> Storage {
        Storage::Memory {
            store: Arc::new(InMemory::new()),
        }
    }

    /// A URL that lets whoever holds it PUT a file at the key until it
    /// expires, with this content type, which is signed, so the bucket
    /// refuses another. With a checksum, the base64 SHA-256 the PUT
    /// carries in `CHECKSUM_SHA256_HEADER`, signed too, the bucket
    /// refuses any other bytes. The size cannot be signed into a PUT URL
    /// here, so it is checked once the file is there.
    pub async fn upload_url(
        &self,
        key: &str,
        content_type: &str,
        checksum_sha256: Option<&str>,
        lifetime: Duration,
    ) -> Result<String, StorageError> {
        match self {
            Storage::S3 { store, .. } => {
                let header = |value: &str| {
                    HeaderValue::from_str(value).map_err(|error| StorageError(error.to_string()))
                };
                let mut options = SignedUrlOptions::new().with_signed_header(
                    object_store::signer::HeaderName::from_static("content-type"),
                    header(content_type)?,
                );
                if let Some(checksum_sha256) = checksum_sha256 {
                    options = options.with_signed_header(
                        object_store::signer::HeaderName::from_static(CHECKSUM_SHA256_HEADER),
                        header(checksum_sha256)?,
                    );
                }
                let url = store
                    .signed_url_opts(Method::PUT, &Path::from(key), lifetime, &options)
                    .await?;
                Ok(url.to_string())
            }
            Storage::Memory { .. } => Ok(format!("memory:///{key}")),
        }
    }

    /// A URL that lets whoever holds it GET the file at the key until it
    /// expires, for a bucket nobody reads without one. With a
    /// `Content-Disposition`, the bucket answers the file with it, so a
    /// browser saves the file under a name instead of showing it.
    pub async fn download_url(
        &self,
        key: &str,
        lifetime: Duration,
        content_disposition: Option<&str>,
    ) -> Result<String, StorageError> {
        match self {
            Storage::S3 { store, .. } => {
                let mut options = SignedUrlOptions::new();
                if let Some(content_disposition) = content_disposition {
                    options =
                        options.with_query([("response-content-disposition", content_disposition)]);
                }
                let url = store
                    .signed_url_opts(Method::GET, &Path::from(key), lifetime, &options)
                    .await?;
                Ok(url.to_string())
            }
            Storage::Memory { .. } => Ok(format!("memory:///{key}")),
        }
    }

    /// The size of the file at the key, or none when there is no file.
    pub async fn size(&self, key: &str) -> Result<Option<u64>, StorageError> {
        match self.store().head(&Path::from(key)).await {
            Ok(meta) => Ok(Some(meta.size)),
            Err(object_store::Error::NotFound { .. }) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    /// The base64 SHA-256 of the file at the key, as the bucket checked
    /// it on the PUT, or none when there is no file or it came without
    /// one. In memory, worked out from the bytes.
    pub async fn checksum_sha256(&self, key: &str) -> Result<Option<String>, StorageError> {
        match self {
            Storage::S3 { store, http, .. } => {
                let options = SignedUrlOptions::new().with_signed_header(
                    object_store::signer::HeaderName::from_static("x-amz-checksum-mode"),
                    HeaderValue::from_static("ENABLED"),
                );
                let url = store
                    .signed_url_opts(Method::HEAD, &Path::from(key), HEAD_URL_LIFETIME, &options)
                    .await?;
                let response = http
                    .head(url)
                    .header("x-amz-checksum-mode", "ENABLED")
                    .send()
                    .await
                    .map_err(|error| StorageError(error.to_string()))?;
                if response.status() == reqwest::StatusCode::NOT_FOUND {
                    return Ok(None);
                }
                if !response.status().is_success() {
                    return Err(StorageError(format!(
                        "the bucket answered {} for {key}",
                        response.status()
                    )));
                }
                Ok(response
                    .headers()
                    .get(CHECKSUM_SHA256_HEADER)
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_string))
            }
            Storage::Memory { store } => match store.get(&Path::from(key)).await {
                Ok(result) => Ok(Some(BASE64.encode(Sha256::digest(result.bytes().await?)))),
                Err(object_store::Error::NotFound { .. }) => Ok(None),
                Err(error) => Err(error.into()),
            },
        }
    }

    /// The file's first bytes, up to the count, to tell what it is.
    pub async fn first_bytes(
        &self,
        key: &str,
        count: u64,
        size: u64,
    ) -> Result<Vec<u8>, StorageError> {
        let bytes = self
            .store()
            .get_range(&Path::from(key), 0..count.min(size))
            .await?;
        Ok(bytes.to_vec())
    }

    /// The whole file at the key: for a test, since a file is read from
    /// the bucket by whoever holds its URL, never through the server.
    pub async fn get(&self, key: &str) -> Result<Vec<u8>, StorageError> {
        let result = self.store().get(&Path::from(key)).await?;
        Ok(result.bytes().await?.to_vec())
    }

    pub async fn put(&self, key: &str, bytes: Vec<u8>) -> Result<(), StorageError> {
        self.store()
            .put(&Path::from(key), PutPayload::from(bytes))
            .await?;
        Ok(())
    }

    /// Every file whose key starts with the folder, `avatars` say.
    pub async fn list(&self, folder: &str) -> Result<Vec<StoredFile>, StorageError> {
        let files = self
            .store()
            .list(Some(&Path::from(folder)))
            .map_ok(|meta| StoredFile {
                key: meta.location.to_string(),
                last_modified: meta.last_modified,
            })
            .try_collect()
            .await?;
        Ok(files)
    }

    pub async fn delete(&self, key: &str) -> Result<(), StorageError> {
        self.store().delete(&Path::from(key)).await?;
        Ok(())
    }

    /// Where anyone reads the file at the key, in a public bucket.
    pub fn public_url(&self, key: &str) -> Result<String, StorageError> {
        match self {
            Storage::S3 {
                public_url: Some(public_url),
                ..
            } => Ok(format!("{public_url}/{key}")),
            Storage::S3 {
                public_url: None, ..
            } => Err(StorageError("a private bucket has no public URL".into())),
            Storage::Memory { .. } => Ok(format!("memory:///{key}")),
        }
    }

    fn store(&self) -> &dyn ObjectStore {
        match self {
            Storage::S3 { store, .. } => store.as_ref(),
            Storage::Memory { store } => store.as_ref(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A bucket that checks a signed checksum, as the files rely on: run
    /// against Garage, `bin/storage start`, or any other S3 store, with
    /// its settings in the environment:
    ///
    /// ```sh
    /// eval "$(bin/storage env | sed 's/^/export /')"
    /// cargo test -p app_core --features object-storage checksum -- --ignored
    /// ```
    #[tokio::test]
    #[ignore]
    async fn storage_s3_keeps_only_the_bytes_of_a_signed_checksum() {
        let setting = |name: &str| std::env::var(name).expect(name);
        let storage = Storage::s3(S3Settings {
            endpoint: Some(setting("STORAGE_ENDPOINT")),
            region: setting("STORAGE_REGION"),
            bucket: setting("STORAGE_PRIVATE_BUCKET"),
            access_key_id: setting("STORAGE_ACCESS_KEY_ID"),
            secret_access_key: SecretString::from(setting("STORAGE_SECRET_ACCESS_KEY")),
            public_url: None,
        })
        .expect("the bucket");
        let key = format!("tests/{}", uuid::Uuid::now_v7());
        let picked = b"the bytes the person picked".to_vec();
        let checksum = BASE64.encode(Sha256::digest(&picked));
        let url = storage
            .upload_url(&key, "text/plain", Some(&checksum), Duration::from_secs(60))
            .await
            .expect("a URL");
        let http = reqwest::Client::new();
        let put = |bytes: Vec<u8>, checksum: Option<String>| {
            let mut request = http
                .put(&url)
                .header("content-type", "text/plain")
                .body(bytes);
            if let Some(checksum) = checksum {
                request = request.header(CHECKSUM_SHA256_HEADER, checksum);
            }
            request.send()
        };

        let other = b"the bytes somebody else sent".to_vec();
        for (bytes, checksum) in [
            (other.clone(), Some(checksum.clone())),
            (picked.clone(), None),
            (other.clone(), Some(BASE64.encode(Sha256::digest(&other)))),
        ] {
            let response = put(bytes, checksum).await.expect("answered");
            assert!(response.status().is_client_error(), "{}", response.status());
        }
        assert_eq!(storage.checksum_sha256(&key).await.expect("looked"), None);

        let response = put(picked.clone(), Some(checksum.clone()))
            .await
            .expect("answered");
        assert!(response.status().is_success(), "{}", response.status());
        assert_eq!(
            storage.checksum_sha256(&key).await.expect("looked"),
            Some(checksum)
        );
        assert_eq!(storage.get(&key).await.expect("the bytes"), picked);
        storage.delete(&key).await.expect("deleted");
    }
}
