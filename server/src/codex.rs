//! Tells whether OpenAI's Codex CLI is busy or idle in a tmux pane.
//!
//! Like Cursor's `agent`, Codex writes no status file. It writes each chat
//! as it goes to a "rollout" transcript,
//! `~/.codex/sessions/<year>/<month>/<day>/rollout-<time>-<session id>.jsonl`
//! (under `$CODEX_HOME` instead of `~/.codex` when set), and keeps it open.
//! So the status is a guess from the writes to the rollout the process has
//! open (found with `lsof`), see `recent_writes.rs`. The rollout's name ends
//! with the session's id, which `codex_chat.rs` finds it by.
//!
//! Written from Codex's source and docs, not yet checked against a running
//! Codex; see decision 26 in `docs/decisions.md`.

use std::path::{Path, PathBuf};

use crate::agent::{AgentKind, AgentSession};
use crate::agent_chat::home_directory_path;
use crate::recent_writes;

const ROLLOUT_PREFIX: &str = "rollout-";
const ROLLOUT_EXTENSION: &str = ".jsonl";
const SESSIONS_DIRECTORY: &str = "sessions";
/// A session id is a UUID, like `5973b6c0-94b8-487b-a530-2aeb6098ae0e`.
const SESSION_ID_LENGTH: usize = 36;

/// The `codex` process `pid` with its session's id and the status guessed
/// from its rollout. Both are `None` when the process has no rollout open
/// yet, as before the first prompt.
pub async fn get_session(pid: u32) -> AgentSession {
    let rollout = recent_writes::find_open_file(pid, is_rollout).await;
    let write_status = match &rollout {
        Some(rollout) => recent_writes::get_write_status(rollout).await,
        None => None,
    };
    AgentSession {
        kind: AgentKind::Codex,
        pid,
        session_id: rollout.as_deref().and_then(session_id_of),
        tmux_pane_id: None,
        status_updated_at: write_status.as_ref().and_then(|write| write.updated_at),
        status: write_status.map(|write| write.status),
    }
}

/// Where Codex keeps its rollouts: `$CODEX_HOME/sessions`, else
/// `~/.codex/sessions`.
pub fn sessions_directory() -> Option<PathBuf> {
    match std::env::var_os("CODEX_HOME") {
        Some(codex_home) => Some(PathBuf::from(codex_home).join(SESSIONS_DIRECTORY)),
        None => home_directory_path(".codex").map(|codex| codex.join(SESSIONS_DIRECTORY)),
    }
}

/// Whether `path` names a rollout, in a `sessions` directory wherever Codex's
/// home is.
fn is_rollout(path: &str) -> bool {
    let path = Path::new(path);
    let in_sessions = path
        .components()
        .any(|component| component.as_os_str() == SESSIONS_DIRECTORY);
    in_sessions
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(is_rollout_name)
}

pub fn is_rollout_name(file_name: &str) -> bool {
    file_name.starts_with(ROLLOUT_PREFIX) && file_name.ends_with(ROLLOUT_EXTENSION)
}

/// The session's id, the end of the rollout's name:
/// `rollout-2026-11-14T09-30-12-<session id>.jsonl`.
fn session_id_of(rollout: &Path) -> Option<String> {
    let name = rollout
        .file_name()?
        .to_str()?
        .strip_suffix(ROLLOUT_EXTENSION)?;
    let start = name.len().checked_sub(SESSION_ID_LENGTH)?;
    let session_id = name.get(start..)?;
    let is_uuid = session_id
        .chars()
        .all(|character| character.is_ascii_hexdigit() || character == '-')
        && session_id.matches('-').count() == 4;
    is_uuid.then(|| session_id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROLLOUT: &str = "/Users/me/.codex/sessions/2026/11/14/\
                           rollout-2026-11-14T09-30-12-5973b6c0-94b8-487b-a530-2aeb6098ae0e.jsonl";

    #[test]
    fn recognizes_rollouts() {
        assert!(is_rollout(ROLLOUT));
        assert!(!is_rollout("/Users/me/.codex/log/codex-tui.log"));
        assert!(!is_rollout("/Users/me/code/rollout-plan.jsonl"));
    }

    #[test]
    fn reads_the_session_id_from_the_rollouts_name() {
        assert_eq!(
            session_id_of(Path::new(ROLLOUT)).as_deref(),
            Some("5973b6c0-94b8-487b-a530-2aeb6098ae0e")
        );
        assert_eq!(session_id_of(Path::new("rollout-short.jsonl")), None);
    }
}
