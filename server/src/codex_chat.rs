//! Reads a Codex session's chat through the local server, or from its rollout
//! for standalone CLIs (see `codex.rs` for where
//! it is), one `{"timestamp", "type", "payload"}` object per line. The
//! `response_item` lines hold the chat as OpenAI's Responses API items:
//! messages, reasoning summaries, tool calls and their outputs, paired by
//! `call_id`. Of the `event_msg` lines only a stopped turn is shown, since
//! the rest repeat the items. Older versions wrote the items themselves, one
//! per line, which are read too.
//!
//! The legacy rollout is undocumented, so unrecognized lines are skipped.
//! The local reader uses the server's paginated full turn items.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use code_reviewer_core::analysis::markdown_html::render_markdown_html;
use serde_json::Value;

use crate::agent::AgentKind;
use crate::agent_chat::{
    ChatHistory, ChatMessage, ChatNote, ChatNoteKind, ChatPart, ChatRole, MAX_TOOL_SUMMARY_CHARS,
    ToolResult, ToolText, shorten, strip_tags, summarize_chat, text_part, tool_summary, tool_text,
    validate_chat_id,
};
use crate::codex;

/// How a user message Codex wrote starts, with the kind of note it is.
const NOTE_PREFIXES: [(&str, ChatNoteKind); 5] = [
    ("<environment_context>", ChatNoteKind::Context),
    ("<user_instructions>", ChatNoteKind::Context),
    ("# AGENTS.md instructions", ChatNoteKind::Context),
    ("<user_shell_command>", ChatNoteKind::Command),
    ("<turn_aborted>", ChatNoteKind::Interrupted),
];
/// How apply_patch names each file it changes.
const PATCH_FILE_PREFIXES: [&str; 3] = ["*** Add File: ", "*** Update File: ", "*** Delete File: "];

/// The chat of the Codex session `session_id`. Reads the whole rollout, so
/// call it from a blocking task.
pub fn read_chat_history(session_id: &str) -> Result<ChatHistory, String> {
    validate_chat_id(session_id)?;
    let sessions = codex::sessions_directory().ok_or("HOME is not set")?;
    let path = find_rollout(&sessions, session_id)
        .ok_or_else(|| format!("No rollout for Codex session {session_id}"))?;
    let contents = std::fs::read_to_string(&path)
        .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
    let messages = parse_rollout(&contents);
    Ok(ChatHistory {
        agent: AgentKind::Codex,
        path: path.display().to_string(),
        has_tool_results: true,
        summary: summarize_chat(&messages),
        messages,
    })
}

/// Reads paginated history from the existing local server without resuming a turn.
#[cfg(unix)]
pub(crate) async fn read_local_chat_history(session_id: &str) -> Result<ChatHistory, String> {
    let mut reader = crate::codex_local::Reader::connect()
        .await
        .map_err(|error| error.to_string())?;
    let thread = reader
        .read_thread(session_id)
        .await
        .map_err(|error| error.to_string())?;
    if thread.history_mode.as_deref() != Some("paginated") {
        let session_id = session_id.to_string();
        return tokio::task::spawn_blocking(move || read_chat_history(&session_id))
            .await
            .map_err(|error| format!("Reading the Codex rollout failed: {error}"))?;
    }
    let turns = reader
        .read_turns(session_id)
        .await
        .map_err(|error| error.to_string())?;
    tokio::task::spawn_blocking(move || {
        let messages = parse_local_turns(&turns);
        ChatHistory {
            agent: AgentKind::Codex,
            path: thread
                .path
                .unwrap_or_else(|| format!("local Codex thread {}", thread.id)),
            has_tool_results: true,
            summary: summarize_chat(&messages),
            messages,
        }
    })
    .await
    .map_err(|error| format!("Reading local Codex history failed: {error}"))
}

#[cfg(unix)]
fn parse_local_turns(turns: &[Value]) -> Vec<ChatMessage> {
    let mut messages = Vec::new();
    for turn in turns {
        let timestamp = turn["startedAt"]
            .as_i64()
            .and_then(|seconds| chrono::DateTime::from_timestamp(seconds, 0))
            .map(|time| time.to_rfc3339());
        for item in turn["items"].as_array().into_iter().flatten() {
            let Some(id) = item["id"].as_str() else {
                continue;
            };
            let kind = item["type"].as_str().unwrap_or_default();
            if kind == "userMessage" {
                let content: Vec<Value> = item["content"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|input| {
                        let text = match input["type"].as_str()? {
                            "text" => input["text"].as_str()?.to_string(),
                            "localImage" | "localAudio" | "skill" => {
                                input["path"].as_str()?.to_string()
                            }
                            "image" | "audio" => "[media attachment]".to_string(),
                            _ => return None,
                        };
                        Some(serde_json::json!({"type":"input_text","text":text}))
                    })
                    .collect();
                let response = serde_json::json!({"role":"user","content":content});
                if let Some(ChatEntry::Message(message)) =
                    read_message(id, timestamp.clone(), &response)
                {
                    messages.push(message);
                }
                continue;
            }
            let part = match kind {
                "agentMessage" | "plan" => {
                    let Some(part) = text_part(item["text"].as_str().unwrap_or_default()) else {
                        continue;
                    };
                    part
                }
                "reasoning" => {
                    let text = ["summary", "content"]
                        .into_iter()
                        .flat_map(|key| item[key].as_array().into_iter().flatten())
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join("\n\n");
                    if text.is_empty() {
                        continue;
                    }
                    ChatPart::Thinking { text }
                }
                "contextCompaction" => ChatPart::PlainText {
                    text: tool_text("Conversation compacted".into()),
                },
                _ => local_tool_part(id, kind, item),
            };
            messages.push(ChatMessage {
                id: id.to_string(),
                role: ChatRole::Assistant,
                timestamp: timestamp.clone(),
                note: None,
                parts: vec![part],
            });
        }
        if matches!(turn["status"].as_str(), Some("failed" | "interrupted")) {
            messages.push(ChatMessage {
                id: format!("{}-status", turn["id"].as_str().unwrap_or_default()),
                role: ChatRole::Note,
                timestamp,
                note: Some(ChatNote {
                    kind: ChatNoteKind::Interrupted,
                    summary: format!("Turn {}", turn["status"].as_str().unwrap_or_default()),
                }),
                parts: vec![ChatPart::PlainText {
                    text: tool_text(turn["error"].to_string()),
                }],
            });
        }
    }
    messages
}

#[cfg(unix)]
fn local_tool_part(id: &str, kind: &str, item: &Value) -> ChatPart {
    let name = item["tool"]
        .as_str()
        .unwrap_or(match kind {
            "commandExecution" => "Command",
            "fileChange" => "File changes",
            "webSearch" => "Web search",
            "imageView" => "View image",
            _ => kind,
        })
        .to_string();
    let input = item.get("arguments").unwrap_or(item);
    let summary = item
        .get("command")
        .or_else(|| item.get("query"))
        .and_then(Value::as_str)
        .map(|text| shorten(text, MAX_TOOL_SUMMARY_CHARS))
        .unwrap_or_else(|| {
            if kind == "fileChange" {
                let paths = item["changes"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|change| change["path"].as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                shorten(&paths, MAX_TOOL_SUMMARY_CHARS)
            } else {
                codex_tool_summary(input)
            }
        });
    let status = item["status"].as_str();
    let is_error = matches!(status, Some("failed" | "declined"))
        || item["exitCode"].as_i64().is_some_and(|code| code != 0)
        || item["success"] == false
        || !item["error"].is_null();
    let result = if matches!(status, Some("inProgress" | "pending")) {
        None
    } else {
        let output = item
            .get("aggregatedOutput")
            .filter(|value| !value.is_null())
            .or_else(|| item.get("result").filter(|value| !value.is_null()))
            .or_else(|| item.get("contentItems").filter(|value| !value.is_null()))
            .or_else(|| item.get("output"))
            .or_else(|| item.get("changes"))
            .unwrap_or(item);
        Some(ToolResult {
            output: tool_output(output).output,
            is_error,
        })
    };
    ChatPart::ToolUse {
        id: id.into(),
        name,
        summary,
        input: input_text(input),
        result,
    }
}

/// The rollout named after `session_id` in `sessions/<year>/<month>/<day>/`,
/// looked for in the newest days first.
fn find_rollout(sessions: &Path, session_id: &str) -> Option<PathBuf> {
    let suffix = format!("-{session_id}.jsonl");
    let days = list_subdirectories_newest_first(sessions)
        .into_iter()
        .flat_map(|year| list_subdirectories_newest_first(&year))
        .flat_map(|month| list_subdirectories_newest_first(&month));
    days.into_iter().find_map(|day| {
        std::fs::read_dir(day)
            .ok()?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| codex::is_rollout_name(name) && name.ends_with(&suffix))
            })
    })
}

/// Named by number (`2026`, `11`, `14`), so the names sort newest last.
fn list_subdirectories_newest_first(directory: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut directories: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    directories.sort_unstable_by(|a, b| b.cmp(a));
    directories
}

/// What a rollout line holds.
enum RolloutItem<'a> {
    Response(&'a Value),
    Event(&'a Value),
    Compacted(&'a Value),
}

fn rollout_item(line: &Value) -> Option<RolloutItem<'_>> {
    let kind = line.get("type")?.as_str()?;
    match (kind, line.get("payload")) {
        ("response_item", Some(payload)) => Some(RolloutItem::Response(payload)),
        ("event_msg", Some(payload)) => Some(RolloutItem::Event(payload)),
        ("compacted", Some(payload)) => Some(RolloutItem::Compacted(payload)),
        // Like `session_meta` and `turn_context`.
        (_, Some(_)) => None,
        // Older versions wrote the item itself.
        (_, None) => Some(RolloutItem::Response(line)),
    }
}

/// What a response item adds to the chat.
enum ChatEntry {
    Message(ChatMessage),
    /// A part of the agent's turn: its text, reasoning or a tool call.
    AssistantPart(ChatPart),
    ToolOutput {
        call_id: Option<String>,
        result: ToolResult,
    },
}

fn parse_rollout(contents: &str) -> Vec<ChatMessage> {
    let mut messages: Vec<ChatMessage> = Vec::new();
    // Where each tool call is: its message's and its part's index.
    let mut tool_calls: HashMap<String, (usize, usize)> = HashMap::new();

    for (index, line) in contents.lines().enumerate() {
        // The last line can be half written.
        let Ok(line) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let id = format!("line-{index}");
        let timestamp = line
            .get("timestamp")
            .and_then(Value::as_str)
            .map(str::to_string);
        let entry = match rollout_item(&line) {
            Some(RolloutItem::Response(item)) => read_response_item(&id, timestamp.clone(), item),
            Some(RolloutItem::Event(event)) => read_event(&id, timestamp.clone(), event),
            Some(RolloutItem::Compacted(compacted)) => {
                read_compacted(&id, timestamp.clone(), compacted)
            }
            None => None,
        };
        match entry {
            Some(ChatEntry::Message(message)) => messages.push(message),
            Some(ChatEntry::AssistantPart(part)) => {
                add_assistant_part(&mut messages, &mut tool_calls, id, timestamp, part);
            }
            Some(ChatEntry::ToolOutput { call_id, result }) => {
                let tool_call = call_id
                    .as_ref()
                    .and_then(|call_id| tool_calls.get(call_id))
                    .and_then(|&(message, part)| messages.get_mut(message)?.parts.get_mut(part));
                match tool_call {
                    Some(ChatPart::ToolUse { result: slot, .. }) => *slot = Some(result),
                    _ => {
                        let part = ChatPart::ToolResult {
                            tool_use_id: call_id,
                            result,
                        };
                        add_assistant_part(&mut messages, &mut tool_calls, id, timestamp, part);
                    }
                }
            }
            None => {}
        }
    }
    messages
}

/// Adds the part to the agent's message being written, or starts one. A
/// message ends with the agent's text, so its reasoning and tool calls come
/// with the text that follows them.
fn add_assistant_part(
    messages: &mut Vec<ChatMessage>,
    tool_calls: &mut HashMap<String, (usize, usize)>,
    id: String,
    timestamp: Option<String>,
    part: ChatPart,
) {
    let continues_last = messages.last().is_some_and(|last| {
        last.role == ChatRole::Assistant
            && !matches!(last.parts.last(), Some(ChatPart::Text { .. }))
    });
    if !continues_last {
        messages.push(ChatMessage {
            id,
            role: ChatRole::Assistant,
            timestamp,
            note: None,
            parts: Vec::new(),
        });
    }
    let message_index = messages.len() - 1;
    let Some(last) = messages.last_mut() else {
        return;
    };
    if let ChatPart::ToolUse { id, .. } = &part {
        tool_calls.insert(id.clone(), (message_index, last.parts.len()));
    }
    last.parts.push(part);
}

fn read_response_item(id: &str, timestamp: Option<String>, item: &Value) -> Option<ChatEntry> {
    match item.get("type")?.as_str()? {
        "message" => read_message(id, timestamp, item),
        "reasoning" => {
            let text = reasoning_text(item);
            (!text.is_empty()).then_some(ChatEntry::AssistantPart(ChatPart::Thinking { text }))
        }
        "function_call" => {
            let arguments = item.get("arguments").and_then(Value::as_str)?;
            let input = serde_json::from_str(arguments)
                .unwrap_or_else(|_| Value::String(arguments.to_string()));
            tool_call(id, item, &input)
        }
        "custom_tool_call" => tool_call(id, item, item.get("input")?),
        "local_shell_call" => {
            let input = item.get("action").cloned().unwrap_or(Value::Null);
            let call_id = call_id_of(item).unwrap_or_else(|| id.to_string());
            Some(ChatEntry::AssistantPart(ChatPart::ToolUse {
                id: call_id,
                name: "shell".to_string(),
                summary: codex_tool_summary(&input),
                input: input_text(&input),
                result: None,
            }))
        }
        "web_search_call" => {
            let input = item.get("action").cloned().unwrap_or(Value::Null);
            let status = item
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or_default();
            Some(ChatEntry::AssistantPart(ChatPart::ToolUse {
                id: id.to_string(),
                name: "web_search".to_string(),
                summary: codex_tool_summary(&input),
                input: input_text(&input),
                // The search's results aren't written, only its status.
                result: Some(ToolResult {
                    output: tool_text(status.to_string()),
                    is_error: status == "failed",
                }),
            }))
        }
        "function_call_output" | "custom_tool_call_output" | "local_shell_call_output" => {
            Some(ChatEntry::ToolOutput {
                call_id: call_id_of(item),
                result: tool_output(item.get("output").unwrap_or(&Value::Null)),
            })
        }
        _ => None,
    }
}

fn read_message(id: &str, timestamp: Option<String>, item: &Value) -> Option<ChatEntry> {
    let text = message_text(item.get("content")?);
    match item.get("role")?.as_str()? {
        "assistant" => text_part(&text).map(ChatEntry::AssistantPart),
        "user" => {
            let note_kind = NOTE_PREFIXES
                .iter()
                .find(|(prefix, _)| text.trim_start().starts_with(prefix))
                .map(|&(_, kind)| kind);
            match note_kind {
                Some(kind) => note_message(id, timestamp, kind, &text).map(ChatEntry::Message),
                None => Some(ChatEntry::Message(ChatMessage {
                    id: id.to_string(),
                    role: ChatRole::User,
                    timestamp,
                    note: None,
                    parts: vec![text_part(&text)?],
                })),
            }
        }
        // The instructions Codex gives the model, like `developer`'s.
        _ => note_message(id, timestamp, ChatNoteKind::Context, &text).map(ChatEntry::Message),
    }
}

fn read_event(id: &str, timestamp: Option<String>, event: &Value) -> Option<ChatEntry> {
    if event.get("type")?.as_str()? != "turn_aborted" {
        return None;
    }
    let reason = event
        .get("reason")
        .and_then(Value::as_str)
        .unwrap_or("interrupted");
    note_message(id, timestamp, ChatNoteKind::Interrupted, reason).map(ChatEntry::Message)
}

fn read_compacted(id: &str, timestamp: Option<String>, compacted: &Value) -> Option<ChatEntry> {
    let summary = compacted.get("message")?.as_str()?;
    note_message(id, timestamp, ChatNoteKind::CompactSummary, summary).map(ChatEntry::Message)
}

fn note_message(
    id: &str,
    timestamp: Option<String>,
    kind: ChatNoteKind,
    text: &str,
) -> Option<ChatMessage> {
    let text = text.trim();
    let summary = strip_tags(text);
    let first_line = summary
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())?;
    let part = match kind {
        ChatNoteKind::CompactSummary => ChatPart::Text {
            html: render_markdown_html(text),
        },
        _ => ChatPart::PlainText {
            text: tool_text(text.to_string()),
        },
    };
    Some(ChatMessage {
        id: id.to_string(),
        role: ChatRole::Note,
        timestamp,
        note: Some(ChatNote {
            kind,
            summary: shorten(first_line, MAX_TOOL_SUMMARY_CHARS),
        }),
        parts: vec![part],
    })
}

fn tool_call(id: &str, item: &Value, input: &Value) -> Option<ChatEntry> {
    Some(ChatEntry::AssistantPart(ChatPart::ToolUse {
        id: call_id_of(item).unwrap_or_else(|| id.to_string()),
        name: item.get("name")?.as_str()?.to_string(),
        summary: codex_tool_summary(input),
        input: input_text(input),
        result: None,
    }))
}

fn call_id_of(item: &Value) -> Option<String> {
    Some(item.get("call_id")?.as_str()?.to_string())
}

/// The text blocks of a message's `content`: `input_text` and `output_text`.
fn message_text(content: &Value) -> String {
    match content {
        Value::String(text) => text.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter(|block| {
                block
                    .get("type")
                    .and_then(Value::as_str)
                    .is_some_and(|kind| kind.ends_with("text"))
            })
            .filter_map(|block| block.get("text")?.as_str())
            .collect::<Vec<_>>()
            .join("\n\n"),
        _ => String::new(),
    }
}

/// The reasoning's summary, else its text. The full reasoning is usually
/// written encrypted, which can't be shown.
fn reasoning_text(item: &Value) -> String {
    let summary = item.get("summary").map(message_text).unwrap_or_default();
    let text = if summary.trim().is_empty() {
        item.get("content").map(message_text).unwrap_or_default()
    } else {
        summary
    };
    text.trim().to_string()
}

/// A patch as written, any other input as pretty-printed JSON.
fn input_text(input: &Value) -> ToolText {
    match input {
        Value::String(text) => tool_text(text.clone()),
        input => tool_text(serde_json::to_string_pretty(input).unwrap_or_default()),
    }
}

/// One line saying what a tool call does: the script a shell call runs, the
/// first file a patch changes, else what `tool_summary` finds.
fn codex_tool_summary(input: &Value) -> String {
    if let Some(patch) = input
        .as_str()
        .or_else(|| input.get("input").and_then(Value::as_str))
    {
        return patch_summary(patch);
    }
    let command = match input.get("command") {
        Some(Value::Array(words)) => Some(shell_command_text(words)),
        _ => input.get("cmd").and_then(Value::as_str).map(str::to_string),
    };
    match command {
        Some(command) => {
            let first_line = command
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty())
                .unwrap_or_default();
            shorten(first_line, MAX_TOOL_SUMMARY_CHARS)
        }
        None => tool_summary(input),
    }
}

/// `["bash", "-lc", "cargo test"]` as `cargo test`, other commands joined
/// with spaces.
fn shell_command_text(words: &[Value]) -> String {
    let words: Vec<&str> = words.iter().filter_map(Value::as_str).collect();
    match words.as_slice() {
        [_, flag, script] if flag.starts_with('-') && flag.ends_with('c') => script.to_string(),
        words => words.join(" "),
    }
}

fn patch_summary(patch: &str) -> String {
    let first_file = patch.lines().find_map(|line| {
        PATCH_FILE_PREFIXES
            .iter()
            .find_map(|prefix| line.strip_prefix(prefix))
    });
    let first_line = first_file
        .or_else(|| patch.lines().map(str::trim).find(|line| !line.is_empty()))
        .unwrap_or_default();
    shorten(first_line.trim(), MAX_TOOL_SUMMARY_CHARS)
}

/// A tool's output: a string, sometimes JSON with the command's exit code, or
/// an object with its `content` and whether it succeeded.
fn tool_output(output: &Value) -> ToolResult {
    match output {
        Value::String(text) => match serde_json::from_str::<Value>(text) {
            Ok(Value::Object(object)) if object.get("output").is_some_and(Value::is_string) => {
                let exit_code = object
                    .get("metadata")
                    .and_then(|metadata| metadata.get("exit_code"))
                    .and_then(Value::as_i64);
                ToolResult {
                    output: tool_text(
                        object
                            .get("output")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                    ),
                    is_error: exit_code.is_some_and(|code| code != 0),
                }
            }
            _ => ToolResult {
                is_error: reports_failed_exit_code(text),
                output: tool_text(text.clone()),
            },
        },
        Value::Object(object) => {
            let text = object.get("content").map(message_text).unwrap_or_default();
            ToolResult {
                is_error: object.get("success").and_then(Value::as_bool) == Some(false)
                    || reports_failed_exit_code(&text),
                output: tool_text(text),
            }
        }
        other => ToolResult {
            output: tool_text(message_text(other)),
            is_error: false,
        },
    }
}

/// Whether the output's header, like `Exit code: 1` or `Process exited with
/// code 1`, says the command failed.
fn reports_failed_exit_code(output: &str) -> bool {
    output.lines().take(5).any(|line| {
        let code = line
            .strip_prefix("Exit code: ")
            .or_else(|| line.strip_prefix("Process exited with code "));
        code.and_then(|code| code.trim().parse::<i64>().ok())
            .is_some_and(|code| code != 0)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(unix)]
    fn reads_local_messages_and_distinguishes_running_and_failed_tools() {
        let turns = vec![
            serde_json::json!({"id":"turn-1","startedAt":1794710050,"status":"inProgress","items":[
                {"id":"prompt","type":"userMessage","content":[{"type":"text","text":"Fix it"}]},
                {"id":"command","type":"commandExecution","command":"cargo check","status":"completed","exitCode":1,"aggregatedOutput":"compile failed"},
                {"id":"patch","type":"fileChange","status":"inProgress","changes":[]},
                {"id":"answer","type":"agentMessage","text":"Working on it"}
            ]}),
        ];
        let messages = parse_local_turns(&turns);
        assert_eq!(messages.len(), 4);
        assert!(messages.iter().all(|message| message.parts.len() == 1));
        assert!(matches!(&messages[3].parts[0], ChatPart::Text { .. }));
        assert!(
            matches!(&messages[1].parts[0], ChatPart::ToolUse { name, .. } if name == "Command")
        );
        let summary = summarize_chat(&messages);
        assert_eq!(summary.prompts, 1);
        assert_eq!(summary.tool_calls, 2);
        assert_eq!(summary.failed_tool_calls, 1);
        assert_eq!(summary.tool_calls_without_result, 1);
        assert!(messages[0].timestamp.is_some());
        assert!(
            matches!(&messages[1].parts[0], ChatPart::ToolUse {result:Some(ToolResult {is_error:true,output}),..} if output.text == "compile failed")
        );
    }

    const ROLLOUT: &str = r##"{"timestamp":"2026-11-14T14:30:12.001Z","type":"session_meta","payload":{"id":"5973b6c0-94b8-487b-a530-2aeb6098ae0e","cwd":"/repo"}}
{"timestamp":"2026-11-14T14:30:12.002Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"<environment_context>\n  <cwd>/repo</cwd>\n</environment_context>"}]}}
{"timestamp":"2026-11-14T14:30:13.000Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"Run the tests"}]}}
{"timestamp":"2026-11-14T14:30:13.001Z","type":"event_msg","payload":{"type":"user_message","message":"Run the tests"}}
{"timestamp":"2026-11-14T14:30:15.000Z","type":"response_item","payload":{"type":"reasoning","summary":[{"type":"summary_text","text":"**Running tests**"}],"encrypted_content":"gAAA"}}
{"timestamp":"2026-11-14T14:30:15.100Z","type":"response_item","payload":{"type":"function_call","name":"shell","arguments":"{\"command\":[\"bash\",\"-lc\",\"cargo test\"],\"workdir\":\"/repo\"}","call_id":"call_1"}}
{"timestamp":"2026-11-14T14:30:20.000Z","type":"response_item","payload":{"type":"function_call_output","call_id":"call_1","output":"{\"output\":\"1 failed\",\"metadata\":{\"exit_code\":101,\"duration_seconds\":4.2}}"}}
{"timestamp":"2026-11-14T14:30:21.000Z","type":"response_item","payload":{"type":"custom_tool_call","name":"apply_patch","input":"*** Begin Patch\n*** Update File: src/lib.rs\n@@\n-a\n+b\n*** End Patch","call_id":"call_2","status":"completed"}}
{"timestamp":"2026-11-14T14:30:21.500Z","type":"response_item","payload":{"type":"custom_tool_call_output","call_id":"call_2","output":"Exit code: 0\nWall time: 0 seconds\nOutput:\nSuccess."}}
{"timestamp":"2026-11-14T14:30:22.000Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"Fixed the failing test."}]}}
{"timestamp":"2026-11-14T14:31:00.000Z","type":"event_msg","payload":{"type":"turn_aborted","reason":"interrupted"}}
{"timestamp":"2026-11-14T14:31:01.000Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"Hal"##;

    #[test]
    fn reads_prompts_replies_and_tool_calls_with_their_outputs() {
        let messages = parse_rollout(ROLLOUT);

        assert_eq!(messages.len(), 4);
        assert_eq!(messages[0].role, ChatRole::Note);
        assert_eq!(
            messages[0].note,
            Some(ChatNote {
                kind: ChatNoteKind::Context,
                summary: "/repo".to_string()
            })
        );
        assert!(messages[1].is_prompt());
        assert_eq!(
            messages[1].timestamp.as_deref(),
            Some("2026-11-14T14:30:13.000Z")
        );

        let reply = &messages[2];
        assert_eq!(reply.role, ChatRole::Assistant);
        assert_eq!(reply.timestamp.as_deref(), Some("2026-11-14T14:30:15.000Z"));
        let [
            ChatPart::Thinking { text },
            ChatPart::ToolUse {
                name: shell,
                summary: shell_summary,
                result: Some(shell_result),
                ..
            },
            ChatPart::ToolUse {
                name: patch,
                summary: patch_summary,
                result: Some(patch_result),
                ..
            },
            ChatPart::Text { .. },
        ] = reply.parts.as_slice()
        else {
            panic!(
                "expected reasoning, two tool calls and text, got {:?}",
                reply.parts
            );
        };
        assert_eq!(text, "**Running tests**");
        assert_eq!(shell, "shell");
        assert_eq!(shell_summary, "cargo test");
        assert_eq!(shell_result.output.text, "1 failed");
        assert!(shell_result.is_error);
        assert_eq!(patch, "apply_patch");
        assert_eq!(patch_summary, "src/lib.rs");
        assert!(!patch_result.is_error);

        assert_eq!(
            messages[3].note.as_ref().map(|note| note.kind),
            Some(ChatNoteKind::Interrupted)
        );
        let summary = summarize_chat(&messages);
        assert_eq!(summary.tool_calls, 2);
        assert_eq!(summary.failed_tool_calls, 1);
        assert_eq!(summary.tool_calls_without_result, 0);
    }

    #[test]
    fn reads_the_items_older_versions_wrote_without_a_wrapper() {
        let rollout = r#"{"id":"5973b6c0","timestamp":"2025-05-07T17:24:21.123Z"}
{"record_type":"state"}
{"type":"message","role":"user","content":[{"type":"input_text","text":"hello"}]}
{"type":"message","role":"assistant","content":[{"type":"output_text","text":"Hi!"}]}"#;
        let messages = parse_rollout(rollout);

        assert_eq!(messages.len(), 2);
        assert!(messages[0].is_prompt());
        assert_eq!(messages[1].role, ChatRole::Assistant);
    }

    #[test]
    fn summarizes_shell_commands() {
        let bash = serde_json::json!({ "command": ["bash", "-lc", "ls -la\npwd"] });
        assert_eq!(codex_tool_summary(&bash), "ls -la");
        let direct = serde_json::json!({ "command": ["rg", "-n", "todo"] });
        assert_eq!(codex_tool_summary(&direct), "rg -n todo");
        let unified = serde_json::json!({ "cmd": "git status" });
        assert_eq!(codex_tool_summary(&unified), "git status");
    }

    #[test]
    fn tells_failed_outputs_by_their_exit_code() {
        assert!(tool_output(&Value::String("Exit code: 2\nOutput:\nno".into())).is_error);
        assert!(!tool_output(&Value::String("Exit code: 0\nOutput:\nok".into())).is_error);
        assert!(
            tool_output(&serde_json::json!({ "content": "denied", "success": false })).is_error
        );
    }

    #[test]
    fn finds_the_rollout_in_the_newest_days_first() {
        let sessions = std::env::temp_dir().join(format!(
            "code-reviewer-codex-sessions-{}",
            std::process::id()
        ));
        let day = sessions.join("2026/11/14");
        std::fs::create_dir_all(&day).expect("the day directory is created");
        let rollout =
            day.join("rollout-2026-11-14T09-30-12-5973b6c0-94b8-487b-a530-2aeb6098ae0e.jsonl");
        std::fs::write(&rollout, "").expect("the rollout is written");

        assert_eq!(
            find_rollout(&sessions, "5973b6c0-94b8-487b-a530-2aeb6098ae0e"),
            Some(rollout)
        );
        assert_eq!(
            find_rollout(&sessions, "00000000-0000-0000-0000-000000000000"),
            None
        );
        std::fs::remove_dir_all(&sessions).expect("the directory is removed");
    }
}
