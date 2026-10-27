//! Reads the status Claude Code keeps for each running session in
//! `~/.claude/sessions/<pid>.json`.
//!
//! The files are undocumented and may change between Claude Code versions,
//! so every field but `pid` is optional and unreadable files are skipped.
//! Files of exited processes can linger, so only sessions whose process is
//! still running are returned.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::process::Process;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionFile {
    pid: u32,
    session_id: Option<String>,
    name: Option<String>,
    cwd: Option<String>,
    status: Option<String>,
    version: Option<String>,
    /// Like `my-session:@33.%60`: the tmux session, window and pane.
    tmux: Option<String>,
    status_updated_at: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ClaudeCodeSession {
    pub pid: u32,
    pub session_id: Option<String>,
    pub name: Option<String>,
    pub cwd: Option<String>,
    /// As Claude Code reports it. Seen: `busy`, `idle` and `shell`.
    pub status: Option<String>,
    pub version: Option<String>,
    /// The tmux session, when the session runs in tmux.
    pub tmux_session_name: Option<String>,
    /// The tmux pane, like `%60`. Pane ids are unique on one tmux server
    /// only, and the file doesn't say which server.
    pub tmux_pane_id: Option<String>,
    /// The tmux session, window and pane, like `my-session:@33.%60`.
    pub tmux_location: Option<String>,
    /// Milliseconds since the Unix epoch.
    pub status_updated_at: Option<u64>,
}

/// The running Claude Code sessions, most recently updated first.
/// `processes` is the machine's process list, to skip sessions whose
/// process has exited.
pub async fn list_sessions(processes: &[Process]) -> Vec<ClaudeCodeSession> {
    let Some(directory) = sessions_directory() else {
        return Vec::new();
    };
    let Ok(mut entries) = tokio::fs::read_dir(&directory).await else {
        return Vec::new();
    };

    let mut sessions = Vec::new();
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if path.extension().is_none_or(|extension| extension != "json") {
            continue;
        }
        let Ok(contents) = tokio::fs::read(&path).await else {
            continue;
        };
        let Ok(file) = serde_json::from_slice::<SessionFile>(&contents) else {
            continue;
        };
        let is_running = processes
            .iter()
            .any(|process| process.pid == file.pid && process.is_claude_code());
        if is_running {
            sessions.push(ClaudeCodeSession::from(file));
        }
    }

    sessions.sort_by_key(|session| std::cmp::Reverse(session.status_updated_at));
    sessions
}

impl ClaudeCodeSession {
    /// A session known only by its process: one whose status file isn't
    /// written yet, or a Claude Code that writes none.
    pub fn without_status_file(pid: u32) -> Self {
        Self {
            pid,
            session_id: None,
            name: None,
            cwd: None,
            status: None,
            version: None,
            tmux_session_name: None,
            tmux_pane_id: None,
            tmux_location: None,
            status_updated_at: None,
        }
    }
}

impl From<SessionFile> for ClaudeCodeSession {
    fn from(file: SessionFile) -> Self {
        Self {
            pid: file.pid,
            session_id: file.session_id,
            name: file.name,
            cwd: file.cwd,
            status: file.status,
            version: file.version,
            tmux_session_name: file.tmux.as_deref().and_then(session_name),
            tmux_pane_id: file.tmux.as_deref().and_then(pane_id),
            tmux_location: file.tmux,
            status_updated_at: file.status_updated_at,
        }
    }
}

fn sessions_directory() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    Some(PathBuf::from(home).join(".claude/sessions"))
}

/// `my-session:@33.%60` → `my-session`. tmux session names can't contain
/// `:`.
fn session_name(tmux_location: &str) -> Option<String> {
    let (session, _) = tmux_location.split_once(':')?;
    Some(session.to_string())
}

/// `my-session:@33.%60` → `%60`.
fn pane_id(tmux_location: &str) -> Option<String> {
    let (_, pane) = tmux_location.rsplit_once('.')?;
    pane.starts_with('%').then(|| pane.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_session_files() {
        let file: SessionFile = serde_json::from_str(
            r#"{"pid":39100,"sessionId":"f76f","cwd":"/code","status":"busy",
                "tmux":"my.session:@33.%60","statusUpdatedAt":1792782978463,"unknown":1}"#,
        )
        .expect("the file parses");
        let session = ClaudeCodeSession::from(file);

        assert_eq!(session.status.as_deref(), Some("busy"));
        assert_eq!(session.tmux_session_name.as_deref(), Some("my.session"));
        assert_eq!(session.tmux_pane_id.as_deref(), Some("%60"));
        assert_eq!(session.name, None);
    }

    #[test]
    fn reads_pane_ids() {
        assert_eq!(pane_id("tempo:@12.%27").as_deref(), Some("%27"));
        assert_eq!(pane_id("no-pane"), None);
    }
}
