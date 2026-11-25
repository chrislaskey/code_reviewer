//! The app's routes, one module per resource at the path of the core
//! module it calls, `app::posts` for `app_core::app::posts`.

pub mod avatars;
pub mod communities;
pub mod current_community;
pub mod current_user;
pub mod memberships;
pub mod reviews;
pub mod sessions;
pub mod traces;
