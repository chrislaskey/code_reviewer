//! The app's own state the web server keeps while it is up, such as a
//! cache its routes share: one `ServerState`, made with `default()` when
//! the server starts, which a handler takes as `State<Arc<ServerState>>`.
//! Every app edits it to add its own; the template keeps none.

#[cfg(feature = "host")]
use std::sync::Arc;

#[cfg(feature = "host")]
use crate::app::analysis::service::AnalysisService;
#[cfg(feature = "host")]
use crate::app::sessions::host::SessionHost;

/// What the app keeps on the server between requests.
#[derive(Default)]
pub struct ServerState {
    /// The parsed projects reviews and traces share, see `app::reviews`.
    #[cfg(feature = "host")]
    pub analysis: Arc<AnalysisService>,
    /// The tmux sessions and worktrees, see `app::sessions::host`.
    #[cfg(feature = "host")]
    pub sessions: SessionHost,
}
