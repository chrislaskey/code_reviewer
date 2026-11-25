//! The app's commands, one module per resource at the path of the core
//! module it calls, `app::posts` for `app_core::app::posts`, and named as
//! the web server's route module for the same resource.

#[cfg(target_os = "ios")]
pub mod apple_push_registration;
pub mod avatars;
pub mod communities;
pub mod current_community;
pub mod current_user;
pub mod last_visited_page;
pub mod local_users;
pub mod memberships;
pub mod reviews;
#[cfg(desktop)]
pub mod session_notifications;
pub mod sessions;
pub mod traces;
