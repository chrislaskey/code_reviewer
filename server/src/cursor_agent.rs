//! Tells whether Cursor's `agent` is busy or idle in a tmux pane.
//!
//! Unlike Claude Code, `agent` writes no status file. It keeps each chat in
//! a SQLite database under `~/.cursor/chats/`, written to while the agent
//! works. So the status is a guess from the writes to the chat database of
//! the process (found with `lsof`), see `recent_writes.rs`. The database's
//! directory is named after the chat, which names its transcript too.

use std::path::Path;

use crate::agent::{AgentKind, AgentSession};
use crate::recent_writes;

/// The file of a chat database that is written to first.
const CHAT_WRITE_AHEAD_LOG: &str = "store.db-wal";
/// Where `agent` keeps its chats.
const CHATS_DIRECTORY_MARKER: &str = "/.cursor/chats/";

/// The `agent` process `pid` with its chat's id and the status guessed from
/// its chat database. Both are `None` when the process has no chat database
/// open yet.
pub async fn get_session(pid: u32) -> AgentSession {
    let chat_log = recent_writes::find_open_file(pid, is_chat_write_ahead_log).await;
    let write_status = match &chat_log {
        Some(log) => recent_writes::get_write_status(log).await,
        None => None,
    };
    AgentSession {
        kind: AgentKind::CursorAgent,
        pid,
        session_id: chat_log.as_deref().and_then(chat_id_of),
        tmux_pane_id: None,
        status_updated_at: write_status.as_ref().and_then(|write| write.updated_at),
        status: write_status.map(|write| write.status),
    }
}

fn is_chat_write_ahead_log(path: &str) -> bool {
    path.contains(CHATS_DIRECTORY_MARKER) && path.ends_with(CHAT_WRITE_AHEAD_LOG)
}

/// The chat's id: the name of the directory its database is in, like
/// `~/.cursor/chats/<workspace hash>/<chat id>/store.db-wal`.
fn chat_id_of(chat_write_ahead_log: &Path) -> Option<String> {
    let directory = chat_write_ahead_log.parent()?.file_name()?;
    Some(directory.to_str()?.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_the_chat_database() {
        assert!(is_chat_write_ahead_log(
            "/Users/me/.cursor/chats/5f7/5fc/store.db-wal"
        ));
        assert!(!is_chat_write_ahead_log(
            "/Users/me/.cursor/chats/5f7/5fc/store.db"
        ));
        assert!(!is_chat_write_ahead_log("/Users/me/code/store.db-wal"));
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
}
