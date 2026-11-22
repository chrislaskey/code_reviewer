//! `GET /api/servers/current/settings`: what this server offers, its
//! settings with their version, see `app_core::servers::settings`.

use std::sync::Arc;

use axum::Json;
use axum::extract::State;

use app_core::servers::settings::SettingsDocument;

pub async fn get(State(document): State<Arc<SettingsDocument>>) -> Json<SettingsDocument> {
    Json(document.as_ref().clone())
}
