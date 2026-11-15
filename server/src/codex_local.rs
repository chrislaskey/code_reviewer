//! Reads the shared local Codex server without starting or resuming turns.

use std::path::{Path, PathBuf};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::net::UnixStream;
use tokio_tungstenite::{WebSocketStream, tungstenite::Message};

use crate::agent_chat::home_directory_path;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(3);

/// A local transport, protocol, or conversation discovery failure.
#[derive(Debug, thiserror::Error)]
pub(crate) enum ReadError {
    #[error("Could not access the local Codex socket: {0}")]
    Io(#[from] std::io::Error),
    #[error("Local Codex connection failed: {0}")]
    Socket(#[from] tokio_tungstenite::tungstenite::Error),
    #[error("Invalid local Codex response: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Local Codex request timed out")]
    Timeout,
    #[error("{0}")]
    Protocol(String),
}

/// Metadata of a loaded conversation, supplied by the local server.
#[derive(Debug, Deserialize)]
pub(crate) struct Thread {
    /// The exact thread ID used by history requests.
    pub id: String,
    /// The thread's working directory.
    pub cwd: PathBuf,
    /// The persisted transcript, when provided.
    pub path: Option<String>,
    /// Runtime state, including approval flags for active turns.
    pub status: Value,
    /// Unix seconds of the last thread update.
    #[serde(rename = "updatedAt")]
    pub updated_at: Option<u64>,
    /// Main clients use strings; subagent sources carry structured lineage.
    pub source: Value,
    /// Missing on older servers, which use legacy rollouts.
    #[serde(rename = "historyMode")]
    pub history_mode: Option<String>,
}

/// A short-lived read connection; dropping it never resumes or changes a thread.
pub(crate) struct Reader {
    socket: WebSocketStream<UnixStream>,
    next_id: u64,
}

/// The existing daemon's default control socket, under the configured Codex home.
pub(crate) fn socket_path() -> Option<PathBuf> {
    let home = std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| home_directory_path(".codex"))?;
    Some(home.join("app-server-control/app-server-control.sock"))
}

impl Reader {
    /// Connects to the existing daemon and identifies this read client.
    pub async fn connect() -> Result<Self, ReadError> {
        let path = socket_path().ok_or_else(|| ReadError::Protocol("HOME is not set".into()))?;
        Self::connect_to(&path).await
    }

    async fn connect_to(path: &Path) -> Result<Self, ReadError> {
        let (socket, _) = tokio::time::timeout(REQUEST_TIMEOUT, async {
            let stream = UnixStream::connect(path).await?;
            Ok::<_, ReadError>(tokio_tungstenite::client_async("ws://localhost/", stream).await?)
        })
        .await
        .map_err(|_| ReadError::Timeout)??;
        let mut reader = Self { socket, next_id: 1 };
        reader.request("initialize", json!({"clientInfo": {
            "name": "code_reviewer", "title": "Code Reviewer", "version": env!("CARGO_PKG_VERSION")
        }})).await?;
        reader
            .socket
            .send(Message::Text(
                json!({"method":"initialized","params":{}})
                    .to_string()
                    .into(),
            ))
            .await?;
        Ok(reader)
    }

    /// Finds the only loaded root conversation in this worktree, refusing ambiguity.
    pub async fn find_thread(&mut self, directory: &Path) -> Result<Thread, ReadError> {
        let mut cursor = Value::Null;
        let mut threads = Vec::new();
        loop {
            let page = self
                .request("thread/loaded/list", json!({"cursor":cursor}))
                .await?;
            let ids = page["data"]
                .as_array()
                .ok_or_else(|| ReadError::Protocol("Codex omitted loaded thread IDs".into()))?;
            for id in ids {
                let response = self.request("thread/read", json!({"threadId":id})).await?;
                let thread: Thread = serde_json::from_value(response["thread"].clone())?;
                if thread.cwd == directory && !thread.source.is_object() {
                    threads.push(thread);
                }
            }
            let next = page["nextCursor"].clone();
            if next.is_null() {
                break;
            }
            if next == cursor {
                return Err(ReadError::Protocol(
                    "Codex repeated a loaded-thread cursor".into(),
                ));
            }
            cursor = next;
        }
        select_thread(threads, directory)
    }

    /// Reads every persisted turn in chronological order, without subscribing or resuming.
    pub async fn read_turns(&mut self, id: &str) -> Result<Vec<Value>, ReadError> {
        let mut turns = Vec::new();
        let mut cursor = Value::Null;
        loop {
            let page = self.request("thread/turns/list", json!({
                "threadId":id, "cursor":cursor, "limit":100, "sortDirection":"asc", "itemsView":"full"
            })).await?;
            let data = page["data"]
                .as_array()
                .ok_or_else(|| ReadError::Protocol("Codex omitted turn history".into()))?;
            turns.extend(data.iter().cloned());
            let next = page["nextCursor"].clone();
            if next.is_null() {
                break;
            }
            if next == cursor {
                return Err(ReadError::Protocol(
                    "Codex repeated a turn-history cursor".into(),
                ));
            }
            cursor = next;
        }
        Ok(turns)
    }

    /// Reads metadata by exact thread ID.
    pub async fn read_thread(&mut self, id: &str) -> Result<Thread, ReadError> {
        let result = self.request("thread/read", json!({"threadId":id})).await?;
        Ok(serde_json::from_value(result["thread"].clone())?)
    }

    async fn request(&mut self, method: &str, params: Value) -> Result<Value, ReadError> {
        let id = self.next_id;
        self.next_id += 1;
        tokio::time::timeout(REQUEST_TIMEOUT, async {
            self.socket
                .send(Message::Text(
                    json!({"id":id,"method":method,"params":params})
                        .to_string()
                        .into(),
                ))
                .await?;
            while let Some(message) = self.socket.next().await {
                let message = message?;
                let Message::Text(text) = message else {
                    continue;
                };
                let response: Value = serde_json::from_str(&text)?;
                if response["id"] != id {
                    continue;
                }
                if let Some(error) = response.get("error") {
                    return Err(ReadError::Protocol(format!(
                        "Codex {method} failed: {error}"
                    )));
                }
                return response.get("result").cloned().ok_or_else(|| {
                    ReadError::Protocol(format!("Codex {method} omitted its result"))
                });
            }
            Err(ReadError::Protocol("Local Codex connection closed".into()))
        })
        .await
        .map_err(|_| ReadError::Timeout)?
    }
}

fn select_thread(mut threads: Vec<Thread>, directory: &Path) -> Result<Thread, ReadError> {
    if threads.len() > 1 {
        return Err(ReadError::Protocol(format!(
            "Multiple loaded Codex conversations use {}; cannot identify this pane's chat",
            directory.display()
        )));
    }
    threads.pop().ok_or_else(|| {
        ReadError::Protocol(format!(
            "No loaded local Codex conversation uses {}",
            directory.display()
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn reads_paginated_history_without_mutating_the_thread() {
        let directory = tempfile::tempdir().expect("temporary socket directory");
        let path = directory.path().join("codex.sock");
        let listener = tokio::net::UnixListener::bind(&path).expect("bind socket");
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept client");
            let mut socket = tokio_tungstenite::accept_async(stream)
                .await
                .expect("handshake");
            for (method, result) in [
                ("initialize", json!({})),
                ("initialized", Value::Null),
                (
                    "thread/loaded/list",
                    json!({"data":["thread-1"],"nextCursor":null}),
                ),
                (
                    "thread/read",
                    json!({"thread":{"id":"thread-1","cwd":"/worktree","status":{"type":"idle"},"source":"cli"}}),
                ),
                (
                    "thread/turns/list",
                    json!({"data":[{"id":"turn-1"}],"nextCursor":"next-page"}),
                ),
                (
                    "thread/turns/list",
                    json!({"data":[{"id":"turn-2"}],"nextCursor":null}),
                ),
            ] {
                let request = socket.next().await.expect("request").expect("message");
                let request: Value =
                    serde_json::from_str(request.to_text().expect("text")).expect("JSON");
                assert_eq!(request["method"], method);
                if method == "initialized" {
                    continue;
                }
                if method == "thread/turns/list" {
                    assert_eq!(request["params"]["itemsView"], "full");
                    assert_eq!(request["params"]["sortDirection"], "asc");
                    if result["nextCursor"].is_null() {
                        assert_eq!(request["params"]["cursor"], "next-page");
                    }
                }
                socket
                    .send(Message::Text(
                        json!({"method":"thread/status/changed","params":{}})
                            .to_string()
                            .into(),
                    ))
                    .await
                    .expect("notification");
                socket
                    .send(Message::Text(
                        json!({"id":request["id"],"result":result})
                            .to_string()
                            .into(),
                    ))
                    .await
                    .expect("response");
            }
        });
        let mut reader = Reader::connect_to(&path).await.expect("connect");
        let thread = reader
            .find_thread(Path::new("/worktree"))
            .await
            .expect("find thread");
        assert_eq!(thread.id, "thread-1");
        let turns = reader.read_turns(&thread.id).await.expect("read history");
        assert_eq!(turns, vec![json!({"id":"turn-1"}), json!({"id":"turn-2"})]);
        server.await.expect("server task");
    }

    #[test]
    fn refuses_missing_or_ambiguous_conversations() {
        let directory = Path::new("/worktree");
        assert!(
            select_thread(Vec::new(), directory)
                .unwrap_err()
                .to_string()
                .contains("No loaded")
        );
        let threads = ["one", "two"]
            .map(|id| {
                serde_json::from_value(json!({
                    "id":id,"cwd":"/worktree","status":{"type":"idle"},"source":"cli"
                }))
                .expect("thread metadata")
            })
            .into_iter()
            .collect();
        assert!(
            select_thread(threads, directory)
                .unwrap_err()
                .to_string()
                .contains("Multiple loaded")
        );
    }
}
