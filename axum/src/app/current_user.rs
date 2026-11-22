//! The current user's own routes, `GET` and `PUT /api/users/current`:
//! who the request is from, as their own pages show them, and their new
//! name, see `app_core::app::users`.

use axum::extract::State;
use serde::{Deserialize, Serialize};

use app_core::accounts::plans::{self, Plan};
use app_core::app::users::{self, Kind};
use app_core::data::app_users::User;
use app_core::deployment::Deployment;
use app_core::ops::staff::{self, StaffError};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::api_error::ApiError;
use crate::user_auth::CurrentUser;

/// The request's user as their own pages show them: with their
/// account's email, which nobody else is sent, whether they are staff,
/// for the link to the ops app, whether they are a trial user, with
/// when the trial ends, for the banner, and their plan, with the
/// subscription whose payment failed, for that banner.
#[derive(Serialize)]
pub struct CurrentUserShown {
    #[serde(flatten)]
    pub user: User,
    pub email: String,
    pub is_staff: bool,
    pub kind: Kind,
    pub trial_expires_at: Option<DateTime<Utc>>,
    pub plan: Option<Plan>,
    pub payment_failed_subscription_id: Option<Uuid>,
}

/// `GET /api/users/current`: the user the request is from.
pub async fn get(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
) -> Result<axum::Json<CurrentUserShown>, ApiError> {
    shown(&deployment, user).await.map(axum::Json)
}

#[derive(Deserialize)]
pub struct CurrentUserUpdate {
    pub name: String,
}

/// `PUT /api/users/current`: the user's new name.
pub async fn update(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
    axum::Json(update): axum::Json<CurrentUserUpdate>,
) -> Result<axum::Json<CurrentUserShown>, ApiError> {
    let user = users::update(&deployment, &user, &update.name).await?;
    shown(&deployment, user).await.map(axum::Json)
}

async fn shown(deployment: &Deployment, user: User) -> Result<CurrentUserShown, ApiError> {
    let account = app_core::accounts::users::get(deployment, user.id).await?;
    let is_staff = match staff::get(deployment, &user).await {
        Ok(_) => true,
        Err(StaffError::NotStaff) => false,
        Err(error) => return Err(error.into()),
    };
    let kind = match account.trial_expires_at {
        Some(_) => Kind::Trial,
        None => Kind::Full,
    };
    let plan = plans::current(deployment, &user).await?;
    let payment_failed_subscription_id = plans::payment_failed(deployment, &user).await?;
    Ok(CurrentUserShown {
        user,
        email: account.email,
        is_staff,
        kind,
        trial_expires_at: account.trial_expires_at,
        plan,
        payment_failed_subscription_id,
    })
}
