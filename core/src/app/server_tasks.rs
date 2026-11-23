//! The app's own tasks the web server runs while it is up, see
//! `server_tasks`: a task the app adds implements
//! `server_tasks::ServerTask` and is listed here.

use crate::server_tasks::ServerTask;

/// The app's tasks, each started when the server starts.
pub fn list() -> &'static [&'static dyn ServerTask] {
    &[]
}
