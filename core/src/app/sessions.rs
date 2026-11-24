//! Sessions: the tmux sessions the app started in git worktrees on the
//! server's machine, and the coding agents in them. The work is
//! `on_host`, with the server's state in `host`; it needs the `host`
//! feature, on the server alone. A device asks the server instead, `online`.

#[cfg(feature = "host")]
pub mod artifacts;
#[cfg(feature = "host")]
pub mod continue_command;
#[cfg(feature = "host")]
pub mod finished_sessions;
#[cfg(feature = "host")]
pub mod host;
#[cfg(feature = "host")]
pub mod notifications;
#[cfg(feature = "host")]
mod on_host;
#[cfg(feature = "sync-http")]
pub mod online;
#[cfg(feature = "host")]
pub mod private_file;
#[cfg(feature = "host")]
pub mod summaries;

#[cfg(feature = "host")]
pub use on_host::*;
