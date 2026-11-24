//! The app's own tasks the web server runs while it is up, see
//! `server_tasks`: a task the app adds implements
//! `server_tasks::ServerTask` and is listed here.

#[cfg(feature = "host")]
use crate::app::sessions::finished_sessions::FinishedSessionsWatcher;
use crate::server_tasks::ServerTask;

/// The app's tasks, each started when the server starts.
#[cfg(feature = "host")]
pub fn list() -> &'static [&'static dyn ServerTask] {
    &[&FinishedSessionsWatcher]
}

/// None in a build without the host feature, such as the app's.
#[cfg(not(feature = "host"))]
pub fn list() -> &'static [&'static dyn ServerTask] {
    &[]
}
