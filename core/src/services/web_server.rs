//! The web server, as a device reaches it: the `Transport` a sync and a
//! sign-in go through, with the device's token. `Http` is the real one,
//! behind the `sync-http` feature, for the apps; `InProcess` calls the
//! server's own functions in this process, for tests.

use crate::accounts::{self, AccountError, Session};
#[cfg(feature = "sync-http")]
use crate::app::avatars::{AvatarUpdate, AvatarUpload, AvatarUploadNew};
#[cfg(feature = "sync-http")]
use crate::app::communities::CommunityAndMembership;
use crate::app::users::{self, UserError};
use crate::data::app_users::User;
use crate::deployment::Deployment;
#[cfg(feature = "sync-http")]
use crate::servers::settings::{SETTINGS_VERSION_HEADER, SettingsDocument};
#[cfg(feature = "sync-http")]
use crate::servers::{ListedServer, ServerDescription};
#[cfg(feature = "object-storage")]
use crate::sync::bytes::{self, BytesError};
use crate::sync::bytes::{Upload, UploadNew};
use crate::sync::{self, BackfillRequest, Page, PullRequest, PushRequest, PushResponse, SyncError};
#[cfg(feature = "sync-http")]
use crate::sync::{PROTOCOL_VERSION, PROTOCOL_VERSION_HEADER};
#[cfg(feature = "sync-http")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "sync-http")]
use serde_json::Value;
use std::future::Future;
use std::path::Path;
use uuid::Uuid;

/// The server, as a device reaches it, with the device's token.
pub trait Transport {
    fn push(
        &self,
        token: Option<&str>,
        request: PushRequest,
    ) -> impl Future<Output = Result<PushResponse, SyncError>> + Send;
    fn pull(
        &self,
        token: Option<&str>,
        request: PullRequest,
    ) -> impl Future<Output = Result<Page, SyncError>> + Send;
    fn backfill(
        &self,
        token: Option<&str>,
        request: BackfillRequest,
    ) -> impl Future<Output = Result<Page, SyncError>> + Send;
    /// Exchanges the device code a browser sign-in left, with the
    /// verifier the device kept, for the device's session, see
    /// `accounts::sign_in_with_device_code`. A device never sends a
    /// password.
    fn sign_in_with_device_code(
        &self,
        code: &str,
        verifier: &str,
    ) -> impl Future<Output = Result<Session, SyncError>> + Send;
    fn sign_out(&self, token: &str) -> impl Future<Output = Result<(), SyncError>> + Send;
    /// Asks whether the bucket has a row's bytes, and where to send
    /// them when it has not, see `bytes::uploads::create`.
    fn bytes_uploads_create(
        &self,
        token: Option<&str>,
        table: &str,
        request: UploadNew,
    ) -> impl Future<Output = Result<Upload, SyncError>> + Send;
    /// Sends the bytes at the path to where the upload says. The bucket
    /// refusing them is `Refused`; not reaching it, `Transport`.
    fn bytes_upload(
        &self,
        token: Option<&str>,
        table: &str,
        upload: &Upload,
        path: &Path,
    ) -> impl Future<Output = Result<(), SyncError>> + Send;
    /// Fetches the bytes of a row the user sees from the bucket into
    /// the path.
    fn bytes_download(
        &self,
        token: Option<&str>,
        table: &str,
        id: Uuid,
        path: &Path,
    ) -> impl Future<Output = Result<(), SyncError>> + Send;
}

/// The server in this process: its functions called directly, for tests,
/// resolving the token as the web server does.
pub struct InProcess<'a> {
    pub server: &'a Deployment,
}

impl InProcess<'_> {
    async fn user(&self, token: Option<&str>) -> Result<User, SyncError> {
        let token = token.ok_or(SyncError::Unauthorized)?;
        match users::get_by_token(self.server, token).await {
            Ok(user) => Ok(user),
            Err(UserError::UnknownToken) => Err(SyncError::Unauthorized),
            Err(UserError::AccountDeleted) => Err(SyncError::AccountDeleted),
            Err(error) => Err(error.into()),
        }
    }
}

impl Transport for InProcess<'_> {
    async fn push(
        &self,
        token: Option<&str>,
        request: PushRequest,
    ) -> Result<PushResponse, SyncError> {
        let user = self.user(token).await?;
        sync::push(self.server, &user, request).await
    }

    async fn pull(&self, token: Option<&str>, request: PullRequest) -> Result<Page, SyncError> {
        let user = self.user(token).await?;
        sync::pull(self.server, &user, request).await
    }

    async fn backfill(
        &self,
        token: Option<&str>,
        request: BackfillRequest,
    ) -> Result<Page, SyncError> {
        let user = self.user(token).await?;
        sync::backfill(self.server, &user, request).await
    }

    async fn sign_in_with_device_code(
        &self,
        code: &str,
        verifier: &str,
    ) -> Result<Session, SyncError> {
        accounts::sign_in_with_device_code(self.server, code, verifier)
            .await
            .map_err(refused)
    }

    async fn sign_out(&self, token: &str) -> Result<(), SyncError> {
        accounts::sign_out(self.server, token)
            .await
            .map_err(refused)
    }

    #[cfg(feature = "object-storage")]
    async fn bytes_uploads_create(
        &self,
        token: Option<&str>,
        table: &str,
        request: UploadNew,
    ) -> Result<Upload, SyncError> {
        let user = self.user(token).await?;
        bytes::uploads::create(self.server, &user, table, &request)
            .await
            .map_err(refused_bytes)
    }

    /// Puts the bytes where the device's PUT would have.
    #[cfg(feature = "object-storage")]
    async fn bytes_upload(
        &self,
        token: Option<&str>,
        table: &str,
        upload: &Upload,
        path: &Path,
    ) -> Result<(), SyncError> {
        let user = self.user(token).await?;
        let of_table = bytes::of_table(table).map_err(refused_bytes)?;
        let read = tokio::fs::read(path)
            .await
            .map_err(|error| SyncError::Refused(error.to_string()))?;
        bytes::bucket(self.server)
            .map_err(refused_bytes)?
            .put(&bytes::storage_key(of_table, user.id, upload.id), read)
            .await
            .map_err(|error| SyncError::Transport(error.to_string()))
    }

    #[cfg(feature = "object-storage")]
    async fn bytes_download(
        &self,
        token: Option<&str>,
        table: &str,
        id: Uuid,
        path: &Path,
    ) -> Result<(), SyncError> {
        let user = self.user(token).await?;
        let read = bytes::get_from_bucket(self.server, &user, table, id)
            .await
            .map_err(refused_bytes)?;
        tokio::fs::write(path, read)
            .await
            .map_err(|error| SyncError::Malformed(error.to_string()))
    }

    #[cfg(not(feature = "object-storage"))]
    async fn bytes_uploads_create(
        &self,
        _token: Option<&str>,
        _table: &str,
        _request: UploadNew,
    ) -> Result<Upload, SyncError> {
        Err(no_bucket())
    }

    #[cfg(not(feature = "object-storage"))]
    async fn bytes_upload(
        &self,
        _token: Option<&str>,
        _table: &str,
        _upload: &Upload,
        _path: &Path,
    ) -> Result<(), SyncError> {
        Err(no_bucket())
    }

    #[cfg(not(feature = "object-storage"))]
    async fn bytes_download(
        &self,
        _token: Option<&str>,
        _table: &str,
        _id: Uuid,
        _path: &Path,
    ) -> Result<(), SyncError> {
        Err(no_bucket())
    }
}

/// Without the bucket's code compiled in, the server in this process
/// has none, as a server that is down for files would answer.
#[cfg(not(feature = "object-storage"))]
fn no_bucket() -> SyncError {
    SyncError::Transport("the server in this process has no bucket".to_string())
}

/// The server's answer about bytes as the web server would send it:
/// what it will not take is refused in words, a missing or failing
/// bucket is the server failing.
#[cfg(feature = "object-storage")]
fn refused_bytes(error: BytesError) -> SyncError {
    match error {
        BytesError::Database(error) => SyncError::Database(error),
        BytesError::NoFileStore | BytesError::Storage(_) => SyncError::Transport(error.to_string()),
        error => SyncError::Refused(error.to_string()),
    }
}

/// The server's answer as the web server would send it: 401 for a code
/// it does not know, the message for anything else it refused.
fn refused(error: AccountError) -> SyncError {
    match error {
        AccountError::SignInExpired
        | AccountError::CodeDoesNotMatch
        | AccountError::UnknownDeviceCode => SyncError::Unauthorized,
        AccountError::Database(error) => SyncError::Database(error),
        error => SyncError::Refused(error.to_string()),
    }
}

/// The server over HTTP, at its base URL, for the apps; a clone shares
/// its connections.
#[cfg(feature = "sync-http")]
#[derive(Clone)]
pub struct Http {
    client: reqwest::Client,
    /// For a file's content, which the server answers by a redirect to
    /// the bucket: the bucket is asked without the device's token.
    not_following_redirects: reqwest::Client,
    base_url: String,
    /// The settings version the server's last answer carried, shared by
    /// the clones.
    settings_version_seen: std::sync::Arc<std::sync::Mutex<Option<String>>>,
}

/// How long a file's bytes may take to send or fetch, far longer than
/// any other request.
#[cfg(feature = "sync-http")]
const FILE_TRANSFER_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30 * 60);

#[cfg(feature = "sync-http")]
impl Http {
    pub fn new(base_url: &str) -> Result<Http, SyncError> {
        let timeout = std::time::Duration::from_secs(60);
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|error| SyncError::Transport(error.to_string()))?;
        let not_following_redirects = reqwest::Client::builder()
            .timeout(timeout)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|error| SyncError::Transport(error.to_string()))?;
        Ok(Http {
            client,
            not_following_redirects,
            base_url: base_url.trim_end_matches('/').to_string(),
            settings_version_seen: std::sync::Arc::default(),
        })
    }

    /// The settings version the server's last answer carried, if any
    /// did: every sync answer does, see `servers::settings`.
    pub fn settings_version_seen(&self) -> Option<String> {
        self.settings_version_seen
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// Keeps the settings version an answer carried.
    fn settings_version_keep(&self, response: &reqwest::Response) {
        let Some(version) = response
            .headers()
            .get(SETTINGS_VERSION_HEADER)
            .and_then(|value| value.to_str().ok())
        else {
            return;
        };
        *self
            .settings_version_seen
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(version.to_string());
    }

    /// `avatars::uploads::create` on the server, as the token's user.
    pub async fn avatars_uploads_create(
        &self,
        token: &str,
        request: &AvatarUploadNew,
    ) -> Result<AvatarUpload, SyncError> {
        self.post(Some(token), "/api/users/current/avatar/uploads", request)
            .await
    }

    /// `avatars::update` on the server, as the token's user.
    pub async fn avatars_update(
        &self,
        token: &str,
        request: &AvatarUpdate,
    ) -> Result<User, SyncError> {
        self.send(
            reqwest::Method::PUT,
            Some(token),
            "/api/users/current/avatar",
            request,
        )
        .await
    }

    /// `accounts::password::get` on the server, of the token's user.
    pub async fn accounts_password_get(
        &self,
        token: &str,
    ) -> Result<accounts::password::PasswordStatus, SyncError> {
        let path = "/api/accounts/current/password";
        self.send(reqwest::Method::GET, Some(token), path, &())
            .await
    }

    /// An online-only API of the server's, as the token's user, with its
    /// JSON answer as it came: the accounts app's purchasing, which a
    /// device only passes to its pages, see `accounts::online`.
    pub(crate) async fn json(
        &self,
        method: reqwest::Method,
        token: &str,
        path: &str,
        body: Option<&Value>,
    ) -> Result<Value, SyncError> {
        match body {
            Some(body) => self.send(method, Some(token), path, body).await,
            None => self.send(method, Some(token), path, &()).await,
        }
    }

    /// `accounts::password::update` on the server, as the token's user.
    pub async fn accounts_password_update(
        &self,
        token: &str,
        request: &accounts::password::PasswordUpdate,
    ) -> Result<(), SyncError> {
        let path = "/api/accounts/current/password";
        self.send(reqwest::Method::PUT, Some(token), path, request)
            .await
    }

    /// What the server says it is, `/api/servers/current`, asked of a
    /// server before signing in to it.
    pub async fn servers_current_get(&self) -> Result<ServerDescription, SyncError> {
        self.send(reqwest::Method::GET, None, "/api/servers/current", &())
            .await
    }

    /// What the server offers, `/api/servers/current/settings`, with
    /// their version.
    pub async fn servers_current_settings_get(&self) -> Result<SettingsDocument, SyncError> {
        let path = "/api/servers/current/settings";
        self.send(reqwest::Method::GET, None, path, &()).await
    }

    /// Every community the token's user sees on the server, with their
    /// membership when they are in it, `/api/communities`.
    pub async fn communities_list(
        &self,
        token: &str,
    ) -> Result<Vec<CommunityAndMembership>, SyncError> {
        self.send(reqwest::Method::GET, Some(token), "/api/communities", &())
            .await
    }

    /// The server this server lists by the name at the origin, or none,
    /// `/api/servers/:name?url=`, asked of the default server.
    pub async fn servers_get(
        &self,
        name: &str,
        url: &str,
    ) -> Result<Option<ListedServer>, SyncError> {
        let response = self
            .client
            .get(format!("{}/api/servers/{name}", self.base_url))
            .query(&[("url", url)])
            .send()
            .await
            .map_err(|error| SyncError::Transport(error.to_string()))?;
        let status = response.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !status.is_success() {
            return Err(SyncError::Transport(format!(
                "the server refused /api/servers/{name} with {status}"
            )));
        }
        response
            .json()
            .await
            .map(Some)
            .map_err(|error| SyncError::Malformed(error.to_string()))
    }

    async fn post<Request: Serialize, Response: serde::de::DeserializeOwned>(
        &self,
        token: Option<&str>,
        path: &str,
        request: &Request,
    ) -> Result<Response, SyncError> {
        self.send(reqwest::Method::POST, token, path, request).await
    }

    async fn send<Request: Serialize, Response: serde::de::DeserializeOwned>(
        &self,
        method: reqwest::Method,
        token: Option<&str>,
        path: &str,
        request: &Request,
    ) -> Result<Response, SyncError> {
        let mut builder = self
            .client
            .request(method, format!("{}{path}", self.base_url))
            .header(PROTOCOL_VERSION_HEADER, PROTOCOL_VERSION.to_string());
        if let Some(token) = token {
            builder = builder.bearer_auth(token);
        }
        let response = builder
            .json(request)
            .send()
            .await
            .map_err(|error| SyncError::Transport(error.to_string()))?;
        self.settings_version_keep(&response);
        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(SyncError::Unauthorized);
        }
        if status == reqwest::StatusCode::GONE {
            return Err(SyncError::AccountDeleted);
        }
        if status == reqwest::StatusCode::UPGRADE_REQUIRED {
            return Err(protocol_version_error(response).await);
        }
        if !status.is_success() {
            let answer: Option<ErrorAnswer> = response.json().await.ok();
            return Err(match answer {
                Some(answer) if status.is_client_error() => SyncError::Refused(answer.message()),
                _ => SyncError::Transport(format!("the server refused {path} with {status}")),
            });
        }
        if status == reqwest::StatusCode::NO_CONTENT {
            return serde_json::from_value(Value::Null)
                .map_err(|error| SyncError::Malformed(error.to_string()));
        }
        response
            .json()
            .await
            .map_err(|error| SyncError::Malformed(error.to_string()))
    }

    async fn delete(&self, token: &str, path: &str) -> Result<(), SyncError> {
        let response = self
            .client
            .delete(format!("{}{path}", self.base_url))
            .bearer_auth(token)
            .send()
            .await
            .map_err(|error| SyncError::Transport(error.to_string()))?;
        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(SyncError::Unauthorized);
        }
        if status == reqwest::StatusCode::GONE {
            return Err(SyncError::AccountDeleted);
        }
        if !status.is_success() {
            return Err(SyncError::Transport(format!(
                "the server refused {path} with {status}"
            )));
        }
        Ok(())
    }
}

/// A 426's error, in the server's words.
#[cfg(feature = "sync-http")]
async fn protocol_version_error(response: reqwest::Response) -> SyncError {
    let answer: Option<ErrorAnswer> = response.json().await.ok();
    SyncError::ProtocolVersion(answer.map_or_else(
        || "This server syncs with another version of the app".to_string(),
        ErrorAnswer::message,
    ))
}

/// The web server's error answer, `{"error": "..."}`, with the fields
/// that were wrong when a form's were, `{"errors": {...}}`.
#[cfg(feature = "sync-http")]
#[derive(Deserialize)]
struct ErrorAnswer {
    error: String,
    #[serde(default)]
    errors: Option<crate::helpers::field_errors::FieldErrors>,
}

#[cfg(feature = "sync-http")]
impl ErrorAnswer {
    /// The fields' messages when there are any, else the error.
    fn message(self) -> String {
        match self.errors {
            Some(errors) => errors.to_string(),
            None => self.error,
        }
    }
}

#[cfg(feature = "sync-http")]
impl Transport for Http {
    async fn push(
        &self,
        token: Option<&str>,
        request: PushRequest,
    ) -> Result<PushResponse, SyncError> {
        self.post(token, "/api/sync/push", &request).await
    }

    async fn pull(&self, token: Option<&str>, request: PullRequest) -> Result<Page, SyncError> {
        self.post(token, "/api/sync/pull", &request).await
    }

    async fn backfill(
        &self,
        token: Option<&str>,
        request: BackfillRequest,
    ) -> Result<Page, SyncError> {
        self.post(token, "/api/sync/backfill", &request).await
    }

    async fn sign_in_with_device_code(
        &self,
        code: &str,
        verifier: &str,
    ) -> Result<Session, SyncError> {
        self.post(
            None,
            "/api/accounts/device-token",
            &DeviceCodeExchange { code, verifier },
        )
        .await
    }

    async fn sign_out(&self, token: &str) -> Result<(), SyncError> {
        self.delete(token, "/api/accounts/sign-out").await
    }

    async fn bytes_uploads_create(
        &self,
        token: Option<&str>,
        table: &str,
        request: UploadNew,
    ) -> Result<Upload, SyncError> {
        let path = format!("/api/sync/bytes/{table}/uploads");
        self.post(token, &path, &request).await
    }

    /// PUTs the file to the signed URL, streamed from the disk, with the
    /// checksum the bucket checks the bytes against.
    async fn bytes_upload(
        &self,
        _token: Option<&str>,
        _table: &str,
        upload: &Upload,
        path: &Path,
    ) -> Result<(), SyncError> {
        let url = upload
            .upload_url
            .as_deref()
            .ok_or_else(|| SyncError::Malformed("an upload with no URL".to_string()))?;
        let file = tokio::fs::File::open(path)
            .await
            .map_err(|error| SyncError::Refused(error.to_string()))?;
        let size = file
            .metadata()
            .await
            .map_err(|error| SyncError::Refused(error.to_string()))?
            .len();
        let response = self
            .client
            .put(url)
            .timeout(FILE_TRANSFER_TIMEOUT)
            .header(reqwest::header::CONTENT_TYPE, &upload.content_type)
            .header("x-amz-checksum-sha256", &upload.checksum_sha256)
            .header(reqwest::header::CONTENT_LENGTH, size)
            .body(reqwest::Body::from(file))
            .send()
            .await
            .map_err(|error| SyncError::Transport(error.to_string()))?;
        let status = response.status();
        if status.is_client_error() {
            return Err(SyncError::Refused(format!(
                "the storage refused the file with {status}"
            )));
        }
        if !status.is_success() {
            return Err(SyncError::Transport(format!(
                "the storage answered {status}"
            )));
        }
        Ok(())
    }

    /// Asks the server where the row's bytes are, then reads them from
    /// the bucket into the path, a chunk at a time.
    async fn bytes_download(
        &self,
        token: Option<&str>,
        table: &str,
        id: Uuid,
        path: &Path,
    ) -> Result<(), SyncError> {
        use tokio::io::AsyncWriteExt;

        let asked = format!("{}/api/sync/bytes/{table}/{id}/content", self.base_url);
        let mut request = self
            .not_following_redirects
            .get(&asked)
            .header(PROTOCOL_VERSION_HEADER, PROTOCOL_VERSION.to_string());
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        let response = request
            .send()
            .await
            .map_err(|error| SyncError::Transport(error.to_string()))?;
        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(SyncError::Unauthorized);
        }
        if status == reqwest::StatusCode::GONE {
            return Err(SyncError::AccountDeleted);
        }
        if status == reqwest::StatusCode::UPGRADE_REQUIRED {
            return Err(protocol_version_error(response).await);
        }
        if status.is_client_error() {
            return Err(SyncError::Refused(format!(
                "the server has no file {id} for this user"
            )));
        }
        let location = response
            .headers()
            .get(reqwest::header::LOCATION)
            .and_then(|location| location.to_str().ok())
            .filter(|_| status.is_redirection())
            .ok_or_else(|| {
                SyncError::Transport(format!("the server answered {asked} with {status}"))
            })?
            .to_string();

        let mut response = self
            .client
            .get(location)
            .timeout(FILE_TRANSFER_TIMEOUT)
            .send()
            .await
            .map_err(|error| SyncError::Transport(error.to_string()))?;
        if !response.status().is_success() {
            return Err(SyncError::Transport(format!(
                "the storage answered {}",
                response.status()
            )));
        }
        let mut file = tokio::fs::File::create(path)
            .await
            .map_err(|error| SyncError::Malformed(error.to_string()))?;
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|error| SyncError::Transport(error.to_string()))?
        {
            file.write_all(&chunk)
                .await
                .map_err(|error| SyncError::Malformed(error.to_string()))?;
        }
        file.flush()
            .await
            .map_err(|error| SyncError::Malformed(error.to_string()))
    }
}

/// What the exchange of a device code sends.
#[cfg(feature = "sync-http")]
#[derive(Serialize)]
struct DeviceCodeExchange<'a> {
    code: &'a str,
    verifier: &'a str,
}
