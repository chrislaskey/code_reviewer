//! What this server says it is and which other servers it lists, for a
//! device signing in to a server other than its build's default, see
//! `app_core::servers` and `docs/design/multi-server.md`: `GET
//! /api/servers/current` describes this server, see `current`, `GET
//! /api/servers/current/settings` answers what it offers, see
//! `settings`, and `GET
//! /api/servers/:name?url=` answers the listed server with both that name
//! and that origin, or a 404, the same for an unknown name as for a wrong
//! origin, so the list is never published. Neither needs a user.

pub mod current;
pub mod settings;

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use serde::Deserialize;

use app_core::servers::{ListedServer, ListedServers};

use crate::api_error::ApiError;

#[derive(Deserialize)]
pub struct ServerQuery {
    url: String,
}

/// `GET /api/servers/:name?url=`: the listed server with the name and
/// the origin.
pub async fn get(
    State(listed): State<Arc<ListedServers>>,
    Path(name): Path<String>,
    Query(query): Query<ServerQuery>,
) -> Result<Json<ListedServer>, ApiError> {
    listed
        .get(&name, &query.url)
        .cloned()
        .map(Json)
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "No such server is listed"))
}
