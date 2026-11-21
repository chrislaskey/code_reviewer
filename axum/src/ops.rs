//! The ops app on the web server: its API under `/api/ops`; its pages,
//! a build of their own (`svelte/dist-ops`), at `/ops` and under it;
//! and the commands that say who is staff, `app_web ops staff create
//! <email>` and `delete`. All of it is for staff: to anyone else, signed
//! in or not, every path of it is a 404, as a path that does not exist,
//! so nothing tells them there is an ops app. See
//! `docs/design/multi-app.md`.

use std::path::Path;

use axum::extract::{Request, State};
use axum::http::{Method, StatusCode, Uri};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Extension, Json, Router};
use tower_http::services::{ServeDir, ServeFile};

use app_core::data::ops_staff::Staff;
use app_core::deployment::Deployment;
use app_core::ops::staff::{self, StaffError};

use crate::api_error::ApiError;
use crate::current_user::CurrentUser;
use crate::router::{AppState, api_not_found};

/// The API's routes, for staff.
pub fn api(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/api/ops/current-staff", get(current_staff))
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

/// `GET /api/ops/current-staff`: the staff row of the request's user.
async fn current_staff(Extension(staff): Extension<Staff>) -> Json<Staff> {
    Json(staff)
}

impl From<StaffError> for ApiError {
    fn from(error: StaffError) -> Self {
        let status = match error {
            StaffError::NotStaff => StatusCode::NOT_FOUND,
            StaffError::NoAccount { .. } => StatusCode::NOT_FOUND,
            StaffError::AccountUser(error) => return error.into(),
            StaffError::ServerOnly | StaffError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        ApiError::new(status, error.to_string())
    }
}

/// `ops staff create <email>` and `ops staff delete <email>`.
pub async fn command(deployment: &Deployment, words: &[&str]) -> Result<(), String> {
    match words {
        ["staff", "create", email] => {
            staff::create(deployment, email)
                .await
                .map_err(|error| error.to_string())?;
            println!("{email} is staff");
            Ok(())
        }
        ["staff", "delete", email] => {
            staff::delete(deployment, email)
                .await
                .map_err(|error| error.to_string())?;
            println!("{email} is staff no longer");
            Ok(())
        }
        _ => Err("commands: ops staff create <email>, ops staff delete <email>".to_string()),
    }
}
