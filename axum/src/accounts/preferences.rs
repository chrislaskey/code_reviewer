//! The current user's preferences, `GET` and `PUT
//! /api/accounts/current/preferences`: the language the pages are shown
//! in, one key at a time, see `app_core::accounts::preferences`.

use axum::Json;
use axum::extract::State;
use serde::Deserialize;
use serde_json::Value;

use app_core::accounts::preferences::{self, Preferences};
use app_core::deployment::Deployment;

use crate::api_error::ApiError;
use crate::user_auth::CurrentUser;

/// One preference to set: its key and its value, null for the default.
#[derive(Deserialize)]
pub struct PreferenceUpdate {
    pub key: String,
    pub value: Value,
}

/// `GET /api/accounts/current/preferences`: the user's preferences.
pub async fn get(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
) -> Result<Json<Preferences>, ApiError> {
    Ok(Json(preferences::get(&deployment, &user).await?))
}

/// `PUT /api/accounts/current/preferences`: one preference, then all of them.
pub async fn update(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
    Json(update): Json<PreferenceUpdate>,
) -> Result<Json<Preferences>, ApiError> {
    let updated = preferences::update(&deployment, &user, &update.key, update.value).await?;
    Ok(Json(updated))
}
