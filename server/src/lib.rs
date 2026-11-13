//! Code Reviewer's backend: everything the desktop and mobile apps ask of
//! the host computer, with no dependency on Tauri.
//!
//!   * Reviews: what a git comparison changed in an Elixir or Rust
//!     repository, computed by the `code_reviewer_core` crate.
//!   * Traces: the tests in the head of a comparison, and the calls below
//!     one test walked top to bottom. Also computed by the core crate.
//!   * Sessions (`sessions`): the tmux sessions the app started, named
//!     `cr-<name>`, with the agent (Claude Code or Cursor) status and chat in
//!     each, and its git worktrees without a session.
//!
//! [`Backend`] holds the state they share and has one method per command,
//! served as HTTP routes by [`localhost_router`] and [`tailnet_router`].
//! [`start`] runs the first on a localhost listener that requires the
//! desktop app's token, and the second, while remote access is on, on this
//! computer's Tailscale address for paired devices.
//! See decision 25 in `docs/decisions.md` for where this is going.

mod access;
mod agent;
mod agent_chat;
mod analysis;
mod backend;
mod claude_code;
mod claude_code_chat;
mod cursor_agent;
mod cursor_agent_chat;
mod git;
mod git_worktree;
mod http_api;
mod paired_devices;
mod pairing_code;
mod process;
mod remote_access;
pub mod requests;
mod sessions;
mod start;
mod tailnet_listener;
mod tailscale;
mod terminal_app;
mod tmux;
mod tokens;

pub use agent_chat::ChatHistory;
pub use backend::Backend;
pub use http_api::{ApiError, localhost_router, tailnet_router};
pub use paired_devices::PairedDevice;
pub use pairing_code::PairingCode;
pub use remote_access::{CreatedPairedDevice, RemoteAccessStatus};
pub use sessions::{DetectedSettings, SessionList, SessionStatus};
pub use start::{RunningServer, ServerConfig, ServerError, start};
pub use terminal_app::ShowOutcome;
