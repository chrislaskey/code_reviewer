//! The staff member the request is from, for the ops pages.

use axum::{Extension, Json};

use app_core::data::ops_staff::Staff;

/// `GET /api/ops/current-staff`: the staff row of the request's user.
pub async fn get(Extension(staff): Extension<Staff>) -> Json<Staff> {
    Json(staff)
}
