//! Traces of the tests of a git repository on the server's machine: its
//! tests, marking the ones a comparison added or modified, and the calls
//! below one of them, each project function in full with its lines marked
//! against the base. The work is `app::analysis`, run on a blocking
//! thread; it needs the `host` feature, on the server alone. A device
//! asks the server instead, `online`.

#[cfg(feature = "sync-http")]
pub mod online;

#[cfg(feature = "host")]
pub use on_host::*;

#[cfg(feature = "host")]
mod on_host {
    use std::sync::Arc;

    use thiserror::Error;

    use crate::app::analysis::error::AnalysisError;
    use crate::app::analysis::service::{
        AnalysisService, GetTestCallStackRequest, GetTestCallStackResponse, ListTestsRequest,
        ListTestsResponse,
    };

    #[derive(Debug, Error)]
    pub enum TraceError {
        #[error(transparent)]
        Analysis(#[from] AnalysisError),
        #[error("the trace stopped before it finished: {0}")]
        Interrupted(String),
    }

    /// Every test in the head of a comparison, marking the ones it added or modified.
    pub async fn tests_list(
        analysis: &Arc<AnalysisService>,
        request: ListTestsRequest,
    ) -> Result<ListTestsResponse, TraceError> {
        run(analysis, move |service| service.list_tests(request)).await
    }

    /// The calls below one test, walked top to bottom.
    pub async fn call_stacks_get(
        analysis: &Arc<AnalysisService>,
        request: GetTestCallStackRequest,
    ) -> Result<GetTestCallStackResponse, TraceError> {
        run(analysis, move |service| {
            service.get_test_call_stack(request)
        })
        .await
    }

    async fn run<T, F>(analysis: &Arc<AnalysisService>, work: F) -> Result<T, TraceError>
    where
        T: Send + 'static,
        F: FnOnce(&AnalysisService) -> Result<T, AnalysisError> + Send + 'static,
    {
        let analysis = Arc::clone(analysis);
        tokio::task::spawn_blocking(move || work(&analysis))
            .await
            .map_err(|error| TraceError::Interrupted(error.to_string()))?
            .map_err(TraceError::from)
    }
}
