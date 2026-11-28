//! Clients for the services the application talks to. Each is an enum
//! of the real client and a memory one that tests and development use.
//! An app lists its own here beside the template's.

#[cfg(feature = "host")]
pub mod agents;
#[cfg(feature = "app-store")]
pub mod app_store;
#[cfg(feature = "host")]
pub mod apple_push;
#[cfg(feature = "remote-backend")]
pub mod backend;
#[cfg(feature = "host")]
pub mod git;
#[cfg(feature = "host")]
pub mod git_status;
#[cfg(feature = "host")]
pub mod git_worktree;
pub mod mailer;
#[cfg(feature = "host")]
pub mod process;
#[cfg(feature = "object-storage")]
pub mod storage;
#[cfg(feature = "stripe")]
pub mod stripe;
#[cfg(feature = "host")]
pub mod terminal_app;
#[cfg(feature = "host")]
pub mod tmux;
pub mod web_server;
