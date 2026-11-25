//! The bytes a device sends and fetches for a synced row, `app_core::sync::bytes`:
//! where to PUT them, `uploads`, and where to read them, for the user its
//! token stands for.

pub mod uploads;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::http::header::{
    CACHE_CONTROL, CONTENT_DISPOSITION, CONTENT_SECURITY_POLICY, CONTENT_TYPE, LOCATION,
    X_CONTENT_TYPE_OPTIONS,
};
use axum::response::{IntoResponse, Response};
use uuid::Uuid;

use app_core::deployment::Deployment;
use app_core::sync::bytes::{self, RowContent};

use crate::api_error::ApiError;
use crate::user_auth::CurrentUser;

/// How long a browser or device may reuse the redirect to bytes: a
/// little less than the signed URL it points at works.
const CONTENT_REDIRECT_SECONDS: u64 = bytes::DOWNLOAD_URL_LIFETIME.as_secs() - 5 * 60;

/// `GET /api/sync/bytes/{table}/{id}/content`: a redirect to the bytes in
/// the bucket, by a signed URL, or the content a row keeps itself.
pub async fn content(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
    Path((table, id)): Path<(String, Uuid)>,
) -> Result<Response, ApiError> {
    if let Some(content) = bytes::row_content(&deployment, &user, &table, id).await? {
        return Ok(content_response(content, None));
    }
    let url = bytes::download_url(&deployment, &user, &table, id, None).await?;
    Ok(content_redirect(url))
}

/// A redirect to bytes at a signed URL, reused while the URL works.
pub fn content_redirect(url: String) -> Response {
    (
        StatusCode::FOUND,
        [
            (LOCATION, url),
            (
                CACHE_CONTROL,
                format!("private, max-age={CONTENT_REDIRECT_SECONDS}"),
            ),
        ],
    )
        .into_response()
}

/// The content a row keeps itself, as its content type, kept as long as
/// a redirect is: it never changes. Never sniffed as another type, and
/// sandboxed, so an SVG opened on its own runs no script.
pub fn content_response(content: RowContent, content_disposition: Option<String>) -> Response {
    let mut response = (
        StatusCode::OK,
        [
            (CONTENT_TYPE, content.content_type),
            (
                CACHE_CONTROL,
                format!("private, max-age={CONTENT_REDIRECT_SECONDS}"),
            ),
            (X_CONTENT_TYPE_OPTIONS, "nosniff".to_string()),
            (CONTENT_SECURITY_POLICY, "sandbox".to_string()),
        ],
        content.content,
    )
        .into_response();
    if let Some(value) = content_disposition.and_then(|value| value.parse().ok()) {
        response.headers_mut().insert(CONTENT_DISPOSITION, value);
    }
    response
}
