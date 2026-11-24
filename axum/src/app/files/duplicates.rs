//! The files the user sees with the same bytes as those picked, see
//! `app_core::app::files::duplicates`.

use axum::Json;
use axum::extract::State;
use serde::Deserialize;

use app_core::app::files::FileWithState;
use app_core::app::files::duplicates;
use app_core::deployment::Deployment;

use crate::api_error::ApiError;
use crate::user_auth::CurrentUser;

#[derive(Deserialize)]
pub struct DuplicatesQuery {
    /// The checksums of the files a person picked.
    pub sha256s: Vec<String>,
}

/// `POST /api/files/duplicates`: the files the user sees with any of
/// these checksums. A read, but a POST, since hundreds of checksums do
/// not fit a URL.
pub async fn list(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
    Json(query): Json<DuplicatesQuery>,
) -> Result<Json<Vec<FileWithState>>, ApiError> {
    Ok(Json(
        duplicates::list(&deployment, &user, &query.sha256s).await?,
    ))
}
