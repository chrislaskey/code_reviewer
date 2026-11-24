//! `GET /api/servers/current`: what this server says it is, its app, its
//! sync's protocol version, its name, its display name and its settings'
//! version.

use axum::Json;
use axum::extract::State;

use app_core::servers::ServerDescription;

pub async fn get(State(description): State<ServerDescription>) -> Json<ServerDescription> {
    Json(description)
}
