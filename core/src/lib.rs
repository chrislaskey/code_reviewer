//! The application: what the web server (`../axum`) and the Tauri desktop
//! and mobile app (`../tauri`) both run, organized as the project owner's
//! Elixir applications are, see `docs/design/code-organization.md`:
//!
//! - `data`: each table's schema and low-level context, its baseline
//!   validation and its reads and writes, borrowing the caller's
//!   transaction. Only a context issues SQL, through private adapters.
//! - `services`: clients for the services the application talks to.
//! - `helpers`: generic functions any module may call.
//! - `app`, `accounts`, `ops`: each app's high-level contexts, the
//!   functions the server and the app both call, `posts::list`, which take
//!   the `Deployment`, own the transaction and check the acting user.
//! - `database`, `deployment`: the connection, its migrations, and the
//!   role a deployment plays.
//!
//! Synced writes are events so devices and the server converge. Bulk
//! maintenance exceptions are documented in
//! `docs/design/low-level-contexts.md`.

pub mod accounts;
pub mod app;
pub mod data;
pub mod database;
pub mod deployment;
pub mod helpers;
pub mod ops;
pub mod services;
pub mod sync;
