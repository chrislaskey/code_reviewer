//! The bytes a device sends and fetches for a synced row, `app_core::sync::bytes`:
//! where to PUT them, `uploads`, and where to read them, for the user its
//! token stands for.

pub mod uploads;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::http::header::{CACHE_CONTROL, LOCATION};
use axum::response::{IntoResponse, Response};
use uuid::Uuid;

use app_core::deployment::Deployment;
use app_core::sync::bytes;

use crate::api_error::ApiError;
use crate::user_auth::CurrentUser;

/// How long a browser or device may reuse the redirect to bytes: a
/// little less than the signed URL it points at works.
const CONTENT_REDIRECT_SECONDS: u64 = bytes::DOWNLOAD_URL_LIFETIME.as_secs() - 5 * 60;

/// `GET /api/sync/bytes/{table}/{id}/content`: a redirect to the bytes in
/// the bucket, by a signed URL.
pub async fn content(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
    Path((table, id)): Path<(String, Uuid)>,
) -> Result<Response, ApiError> {
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
