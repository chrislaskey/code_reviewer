//! The files API: `app_core::files` as JSON, for the request's user.
//! `POST /api/files/upload` answers where to PUT a file's bytes, the
//! page or a device sends them to the bucket, and `POST /api/files`
//! writes the row once they are there. `GET /api/files/{id}/content`
//! redirects to a URL the bytes are read from, for an `<img>` or a
//! link. See `docs/design/offline-files-decisions.md`.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::http::header::{CACHE_CONTROL, LOCATION};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use uuid::Uuid;

use app_core::deployment::Deployment;
use app_core::files::{self, File, FileNew, FileUpload, FileUploadNew, FileWithState};

use crate::api_error::ApiError;
use crate::current_user::CurrentUser;

/// How long a browser may reuse the redirect to a file's bytes: a
/// little less than the signed URL it points at works.
const CONTENT_REDIRECT_SECONDS: u64 = files::DOWNLOAD_URL_LIFETIME.as_secs() - 5 * 60;

#[derive(Deserialize)]
pub struct FileFields {
    pub name: String,
    pub community_ids: Vec<Uuid>,
}

#[derive(Deserialize)]
pub struct ListQuery {
    /// Present to list only the files with these bytes.
    pub sha256: Option<String>,
}

#[derive(Deserialize)]
pub struct ContentQuery {
    /// Present to save the file under its name instead of showing it.
    pub download: Option<String>,
}

/// The files the user sees, or with `?sha256=`, those with these bytes.
pub async fn list(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<ListQuery>,
) -> Result<Json<Vec<FileWithState>>, ApiError> {
    Ok(Json(match query.sha256 {
        Some(sha256) => files::list_with_sha256(&deployment, &user, &sha256).await?,
        None => files::list(&deployment, &user).await?,
    }))
}

pub async fn get(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<Json<FileWithState>, ApiError> {
    Ok(Json(files::get(&deployment, &user, id).await?))
}

pub async fn create_upload(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
    Json(request): Json<FileUploadNew>,
) -> Result<Json<FileUpload>, ApiError> {
    Ok(Json(
        files::create_upload(&deployment, &user, &request).await?,
    ))
}

pub async fn create(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
    Json(new): Json<FileNew>,
) -> Result<(StatusCode, Json<File>), ApiError> {
    let file = files::create(&deployment, &user, &new).await?;
    Ok((StatusCode::CREATED, Json(file)))
}

pub async fn update(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<Uuid>,
    Json(fields): Json<FileFields>,
) -> Result<Json<File>, ApiError> {
    Ok(Json(
        files::update(&deployment, &user, id, &fields.name, &fields.community_ids).await?,
    ))
}

pub async fn delete(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    files::delete(&deployment, &user, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// The file's bytes, by a redirect to the bucket: the bytes never
/// change, so the browser may keep the redirect for as long as the URL
/// it points at works.
pub async fn content(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<Uuid>,
    Query(query): Query<ContentQuery>,
) -> Result<Response, ApiError> {
    let url = files::content_url(&deployment, &user, id, query.download.is_some()).await?;
    Ok((
        StatusCode::FOUND,
        [
            (LOCATION, url),
            (
                CACHE_CONTROL,
                format!("private, max-age={CONTENT_REDIRECT_SECONDS}"),
            ),
        ],
    )
        .into_response())
}
