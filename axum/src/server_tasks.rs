//! The app's server tasks, see `app_core::server_tasks`: each started
//! when the server starts, and started again `RESTART_AFTER` after it
//! fails, so a watcher that hit an error keeps watching. A task that
//! returns `Ok` is done. They stop when the server does. Each shares the
//! routes' `ServerState`.

use std::sync::Arc;
use std::time::Duration;

use app_core::app::server_state::ServerState;
use app_core::deployment::Deployment;
use app_core::server_tasks::ServerTask;

/// How long after a task fails it is started again.
const RESTART_AFTER: Duration = Duration::from_secs(10);

/// Starts every task the app lists.
pub fn spawn(deployment: Deployment, state: Arc<ServerState>) {
    for task in app_core::app::server_tasks::list() {
        let deployment = deployment.clone();
        let state = Arc::clone(&state);
        tokio::spawn(
            async move { run_restarting(*task, &deployment, &state, RESTART_AFTER).await },
        );
    }
}

/// Runs the task until it returns `Ok`, logging each failure and
/// starting it again `restart_after` later.
async fn run_restarting(
    task: &dyn ServerTask,
    deployment: &Deployment,
    state: &Arc<ServerState>,
    restart_after: Duration,
) {
    tracing::info!("server task {} started", task.name());
    loop {
        match task.run(deployment, state).await {
            Ok(()) => {
                tracing::info!("server task {} is done", task.name());
                return;
            }
            Err(error) => {
                tracing::warn!(
                    "server task {} failed: {error}; starting it again in {}s",
                    task.name(),
                    restart_after.as_secs()
                );
                tokio::time::sleep(restart_after).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use app_core::server_tasks::TaskFailure;
    use async_trait::async_trait;

    use super::*;

    /// Fails its first two runs, then is done.
    #[derive(Debug, Default)]
    struct FailsTwice {
        runs: AtomicUsize,
    }

    #[async_trait]
    impl ServerTask for FailsTwice {
        fn name(&self) -> &'static str {
            "fails_twice"
        }

        async fn run(
            &self,
            _deployment: &Deployment,
            _state: &Arc<ServerState>,
        ) -> Result<(), TaskFailure> {
            if self.runs.fetch_add(1, Ordering::SeqCst) < 2 {
                return Err("not yet".into());
            }
            Ok(())
        }
    }

    #[tokio::test]
    async fn server_tasks_a_failed_task_starts_again_until_it_is_done() {
        let database = app_core::database::Database::open_sqlite_in_memory()
            .await
            .expect("a database");
        let deployment = Deployment::new(database, app_core::deployment::Role::Server);
        let task = FailsTwice::default();
        run_restarting(&task, &deployment, &Arc::default(), Duration::ZERO).await;
        assert_eq!(task.runs.load(Ordering::SeqCst), 3);
    }
}
