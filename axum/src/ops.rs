//! The ops app on the web server: its API under `/api/ops`; its pages,
//! a build of their own (`svelte/dist-ops`), at `/ops` and under it.
//! Who is staff is said from the command line, see
//! `command_line::ops_staff`. All of it is for staff: to anyone else, signed
//! in or not, every path of it is a 404, as a path that does not exist,
//! so nothing tells them there is an ops app. See
//! `docs/design/multi-app.md`.

pub mod current_staff;

use std::path::Path;

use axum::Router;
use axum::extract::{Request, State};
use axum::http::{Method, StatusCode, Uri};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use tower_http::services::{ServeDir, ServeFile};

use app_core::data::ops_staff::Staff;
use app_core::deployment::Deployment;
use app_core::ops::staff::{self, StaffError};

use crate::api_error::ApiError;
use crate::router::api_not_found;
use crate::state::AppState;
use crate::user_auth::CurrentUser;

/// The API's routes, for staff.
pub fn api(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/api/ops/current-staff", get(current_staff::get))
        .route_layer(middleware::from_fn_with_state(state, api_for_staff))
}

/// The ops pages and their files, for staff: the built files under
/// `/ops/`, and `ops.html`, whose router shows the page at the path,
/// for every other path there.
pub fn pages(ops_dist: &Path, state: AppState) -> Router {
    let files = ServeDir::new(ops_dist).fallback(ServeFile::new(ops_dist.join("ops.html")));
    Router::new()
        .nest_service("/ops", files)
        .layer(middleware::from_fn_with_state(state, pages_for_staff))
}

/// The request's staff row, or none when it is from nobody or from
/// someone who is not staff.
async fn staff_of(
    deployment: &Deployment,
    user: Result<CurrentUser, ApiError>,
) -> Result<Option<Staff>, ApiError> {
    let Ok(CurrentUser(user)) = user else {
        return Ok(None);
    };
    match staff::get(deployment, &user).await {
        Ok(staff) => Ok(Some(staff)),
        Err(StaffError::NotStaff) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

/// Lets an API request from staff through, with their `Staff` for the
/// handler; anyone else gets the 404 of an API path with no route.
async fn api_for_staff(
    State(deployment): State<Deployment>,
    user: Result<CurrentUser, ApiError>,
    method: Method,
    uri: Uri,
    mut request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let Some(staff) = staff_of(&deployment, user).await? else {
        return Err(api_not_found(method, uri).await);
    };
    request.extensions_mut().insert(staff);
    Ok(next.run(request).await)
}

/// Lets a request for an ops page or file from staff through; anyone
/// else gets a bare 404.
async fn pages_for_staff(
    State(deployment): State<Deployment>,
    user: Result<CurrentUser, ApiError>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    match staff_of(&deployment, user).await? {
        Some(_) => Ok(next.run(request).await),
        None => Ok(StatusCode::NOT_FOUND.into_response()),
    }
}
