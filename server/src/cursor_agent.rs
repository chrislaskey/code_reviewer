//! Tells whether Cursor's `agent` is busy or idle in a tmux pane.
//!
//! Unlike Claude Code, `agent` writes no status file. It keeps each chat in
//! a SQLite database under `~/.cursor/chats/`, written to while the agent
//! works. So the status is a guess: the chat database of the process (found
//! with `lsof`) written to in the last `BUSY_WINDOW` means busy, else idle.
//! A long tool call that writes nothing reads as idle. The database's
//! directory is named after the chat, which names its transcript too.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::process::Command;

use crate::agent::{AgentKind, AgentSession};

/// The file of a chat database that is written to first.
const CHAT_WRITE_AHEAD_LOG: &str = "store.db-wal";
/// Where `agent` keeps its chats.
const CHATS_DIRECTORY_MARKER: &str = "/.cursor/chats/";
/// A bit more than the pages' refresh interval, so a refresh doesn't fall
/// between two writes.
const BUSY_WINDOW: Duration = Duration::from_secs(10);

/// The `agent` process `pid` with its chat's id and the status guessed from
/// its chat database. Both are `None` when the process has no chat database
/// open yet.
pub async fn get_session(pid: u32) -> AgentSession {
    let chat_log = find_open_chat_write_ahead_log(pid).await;
    let modified = match &chat_log {
        Some(log) => last_write(log).await,
        None => None,
    };
    AgentSession {
        kind: AgentKind::CursorAgent,
        pid,
        session_id: chat_log.as_deref().and_then(chat_id_of),
        tmux_pane_id: None,
        status: modified.map(|modified| status_of(modified, SystemTime::now()).to_string()),
        status_updated_at: modified.and_then(milliseconds_since_epoch),
    }
}

async fn find_open_chat_write_ahead_log(pid: u32) -> Option<PathBuf> {
    find_chat_write_ahead_log(&list_open_files(pid).await?)
}

async fn last_write(path: &Path) -> Option<SystemTime> {
    tokio::fs::metadata(path).await.ok()?.modified().ok()
}

/// The chat's id: the name of the directory its database is in, like
/// `~/.cursor/chats/<workspace hash>/<chat id>/store.db-wal`.
fn chat_id_of(chat_write_ahead_log: &Path) -> Option<String> {
    let directory = chat_write_ahead_log.parent()?.file_name()?;
    Some(directory.to_str()?.to_string())
}

/// The output of `lsof -p <pid> -Fn`: one `n<path>` line per open file.
async fn list_open_files(pid: u32) -> Option<String> {
    let output = Command::new("lsof")
        .args(["-p", &pid.to_string(), "-Fn"])
        .output()
        .await
        .ok()?;
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn find_chat_write_ahead_log(open_files: &str) -> Option<PathBuf> {
    open_files
        .lines()
        .filter_map(|line| line.strip_prefix('n'))
        .find(|path| path.contains(CHATS_DIRECTORY_MARKER) && path.ends_with(CHAT_WRITE_AHEAD_LOG))
        .map(|path| Path::new(path).to_path_buf())
}

fn status_of(last_write: SystemTime, now: SystemTime) -> &'static str {
    let age = now.duration_since(last_write).unwrap_or_default();
    if age <= BUSY_WINDOW { "busy" } else { "idle" }
}

fn milliseconds_since_epoch(time: SystemTime) -> Option<u64> {
    let duration = time.duration_since(UNIX_EPOCH).ok()?;
    u64::try_from(duration.as_millis()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_chat_database_among_open_files() {
        let open_files = "p55812\nn/Users/me/code\nn/Users/me/.cursor/chats/5f7/5fc/store.db\n\
                          n/Users/me/.cursor/chats/5f7/5fc/store.db-wal\nn/dev/ttys000\n";
        assert_eq!(
            find_chat_write_ahead_log(open_files),
            Some(PathBuf::from(
                "/Users/me/.cursor/chats/5f7/5fc/store.db-wal"
            ))
        );
        assert_eq!(find_chat_write_ahead_log("p1\nn/dev/null\n"), None);
    }

    #[test]
    fn names_the_chat_after_its_databases_directory() {
        assert_eq!(
            chat_id_of(Path::new(
                "/Users/me/.cursor/chats/5f7/e4835e25-7082/store.db-wal"
            )),
            Some("e4835e25-7082".to_string())
        );
    }

    #[test]
    fn is_busy_only_after_a_recent_write() {
        let now = UNIX_EPOCH + Duration::from_secs(1_000);
        assert_eq!(status_of(now - Duration::from_secs(2), now), "busy");
        assert_eq!(status_of(now - Duration::from_secs(60), now), "idle");
        assert_eq!(status_of(now + Duration::from_secs(1), now), "busy");
    }
}
