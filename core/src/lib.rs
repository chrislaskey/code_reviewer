//! The application: what the web server (`../axum`) and the Tauri desktop
//! and mobile app (`../tauri`) both run. One module per resource, with
//! `list`, `get`, `create`, `update` and `delete` functions called through
//! the module, `posts::list`, that return plain data the UI shows. They
//! read and write the `Deployment` that opened the database, see
//! `deployment`; the server and the app only translate between their
//! transport and these functions.
//!
//! Application entry points own transactions and enforce the acting user's
//! scope. Internal contexts under `data` borrow those transactions and own
//! private database adapters. Synced business writes use events so devices
//! and the server converge; identity, commerce and infrastructure records
//! use their existing transactional workflows. Bulk maintenance exceptions
//! are documented in `docs/design/low-level-contexts.md`.

pub mod accounts;
pub mod app;
pub mod credentials;
mod data;
pub mod database;
pub mod deployment;
pub mod device_settings;
pub mod field_errors;
pub mod jobs;
pub mod mailer;
pub mod mails;
pub mod ops;
#[cfg(feature = "postgres")]
mod postgres;
mod sqlite;
#[cfg(feature = "object-storage")]
pub mod storage;
pub mod tokens;
pub mod trial_users;

// Preserve existing resource paths while grouping their implementations under app.
pub use app::{
    avatars, communities, conflicts, current_community, events, files, files_communities,
    local_users, memberships, pending_changes, posts, posts_communities, purge, sync, users,
};
