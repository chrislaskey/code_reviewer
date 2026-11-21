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
pub mod data;
pub mod database;
pub mod deployment;
pub mod helpers;
pub mod ops;
#[cfg(feature = "postgres")]
mod postgres;
pub mod services;
mod sqlite;
