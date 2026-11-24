//! The files API: `app_core::app::files` as JSON, for the request's user.
//! `POST /api/files/uploads`, `uploads`, answers where to PUT a file's bytes, the
//! page or a device sends them to the bucket, and `POST /api/files`
//! writes the row once they are there. `GET /api/files/{id}/content`
//! redirects to a URL the bytes are read from, for an `<img>` or a
//! link. See `docs/design/offline-files-decisions.md`.

pub mod duplicates;
pub mod uploads;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::Response;
use serde::Deserialize;
use uuid::Uuid;

use app_core::app::files::{self, FileFilter, FileList, FileNew, FileWithState};
use app_core::data::app_files::File;
use app_core::deployment::Deployment;

use crate::api_error::ApiError;
use crate::sync;
use crate::user_auth::CurrentUser;

#[derive(Deserialize)]
pub struct FileFields {
    pub name: String,
    pub community_ids: Vec<Uuid>,
}

/// `FileFilter` in a URL's query, its content types joined by commas,
/// as `image/png,image/jpeg`.
#[derive(Deserialize)]
pub struct FileListQuery {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub content_types: String,
    #[serde(default)]
    pub offset: u32,
    pub limit: u32,
}

impl From<FileListQuery> for FileFilter {
    fn from(query: FileListQuery) -> FileFilter {
        FileFilter {
            name: query.name,
            content_types: query
                .content_types
                .split(',')
                .map(str::trim)
                .filter(|content_type| !content_type.is_empty())
                .map(str::to_string)
                .collect(),
            offset: query.offset,
            limit: query.limit,
        }
    }
}

#[derive(Deserialize)]
pub struct ContentQuery {
    /// Present to save the file under its name instead of showing it.
    pub download: Option<String>,
}

/// `GET /api/files`: a page of the files the user sees that pass the
/// filter, and how many pass in all.
pub async fn list(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<FileListQuery>,
) -> Result<Json<FileList>, ApiError> {
    let filter = FileFilter::from(query);
    Ok(Json(files::list(&deployment, &user, &filter).await?))
}

pub async fn get(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<Json<FileWithState>, ApiError> {
    Ok(Json(files::get(&deployment, &user, id).await?))
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
    Ok(sync::bytes::content_redirect(url))
}
