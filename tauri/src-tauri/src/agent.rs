//! The coding agent running in a tmux session, Claude Code or Cursor's
//! `agent`, with the status the pages show. How each one's status is found
//! is in `claude_code.rs` and `cursor_agent.rs`.

use serde::Serialize;

use crate::claude_code::ClaudeCodeSession;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentKind {
    ClaudeCode,
    CursorAgent,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AgentSession {
    pub kind: AgentKind,
    pub pid: u32,
    /// The agent's own id for its chat, when it reports one. Claude Code's
    /// names its transcript (see `claude_code_chat.rs`).
    pub session_id: Option<String>,
    /// The tmux pane the agent runs in, like `%60`, to send prompts to.
    pub tmux_pane_id: Option<String>,
    /// `busy` or `idle` (Claude Code also reports `shell`), or `None` when
    /// it can't be told.
    pub status: Option<String>,
    /// Milliseconds since the Unix epoch.
    pub status_updated_at: Option<u64>,
}

impl From<ClaudeCodeSession> for AgentSession {
    fn from(session: ClaudeCodeSession) -> Self {
        Self {
            kind: AgentKind::ClaudeCode,
            pid: session.pid,
            session_id: session.session_id,
            tmux_pane_id: session.tmux_pane_id,
            status: session.status,
            status_updated_at: session.status_updated_at,
        }
    }
}
