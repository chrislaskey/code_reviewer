//! The app's commands, one module per resource at the path of the core
//! module it calls, `app::posts` for `app_core::app::posts`, and named as
//! the web server's route module for the same resource.

pub mod avatars;
pub mod communities;
pub mod conflicts;
pub mod current_community;
pub mod current_user;
pub mod files;
pub mod local_users;
pub mod memberships;
pub mod pending_changes;
pub mod posts;
