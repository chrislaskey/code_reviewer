//! Tasks the web server runs for as long as it is up, beside the
//! recurring jobs: a watcher that must notice a change within seconds,
//! where a job runs at most every few minutes, see `docs/design/jobs.md`.
//! Each is a `ServerTask`; the template has none of its own, and an
//! app's are listed in `app::server_tasks`. The server starts each when
//! it starts, not for a command on its command line, starts one that
//! failed again after a pause, and stops them all when it stops.

use async_trait::async_trait;

use crate::deployment::Deployment;

/// The error a task ended with, as the server logs it.
pub type TaskFailure = Box<dyn std::error::Error + Send + Sync>;

/// A task that runs for as long as the server does.
#[async_trait]
pub trait ServerTask: Send + Sync + std::fmt::Debug {
    /// Its name in the server's log.
    fn name(&self) -> &'static str;

    /// Runs until the server stops. An error is logged and the task
    /// started again; `Ok` means it has nothing left to do.
    async fn run(&self, deployment: &Deployment) -> Result<(), TaskFailure>;
}
