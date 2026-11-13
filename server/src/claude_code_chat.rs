//! Reads a Claude Code session's chat from the transcript Claude Code writes
//! to `~/.claude/projects/<directory>/<session id>.jsonl`, one JSON object
//! per line.
//!
//! The transcript is undocumented and may change between Claude Code
//! versions, so lines that don't parse, and kinds of lines or content the
//! chat doesn't show, are skipped. The user's and the assistant's messages
//! are kept, with each tool call paired with its result, and so are the
//! lines Claude Code writes as the user's, as notes saying what they are.

use std::collections::HashMap;
use std::path::Path;

use code_reviewer_core::analysis::markdown_html::render_markdown_html;
use serde::Deserialize;
use serde_json::Value;

use crate::agent::AgentKind;
use crate::agent_chat::{
    ChatHistory, ChatMessage, ChatNote, ChatNoteKind, ChatPart, ChatRole, MAX_TOOL_SUMMARY_CHARS,
    ToolResult, content_text, find_in_subdirectories, home_directory_path, shorten, strip_tags,
    summarize_chat, tag_contents, text_part, tool_summary, tool_text, validate_chat_id,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TranscriptLine {
    #[serde(rename = "type")]
    kind: String,
    uuid: Option<String>,
    timestamp: Option<String>,
    message: Option<TranscriptMessage>,
    /// A subagent's message.
    #[serde(default)]
    is_sidechain: bool,
    /// Context Claude Code adds, not typed by the user.
    #[serde(default)]
    is_meta: bool,
    #[serde(default)]
    is_compact_summary: bool,
    /// Who wrote a user line: `human` when the user typed it.
    origin: Option<TranscriptOrigin>,
}

#[derive(Deserialize)]
struct TranscriptOrigin {
    kind: String,
}

#[derive(Deserialize)]
struct TranscriptMessage {
    id: Option<String>,
    content: Value,
}

/// The chat of the Claude Code session `session_id`. Reads the whole
/// transcript, so call it from a blocking task.
pub fn read_chat_history(session_id: &str) -> Result<ChatHistory, String> {
    validate_chat_id(session_id)?;
    let projects = home_directory_path(".claude/projects").ok_or("HOME is not set")?;
    let path = find_in_subdirectories(&projects, Path::new(&format!("{session_id}.jsonl")))
        .ok_or_else(|| format!("No transcript for Claude Code session {session_id}"))?;
    let contents = std::fs::read_to_string(&path)
        .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
    let messages = parse_transcript(&contents);
    Ok(ChatHistory {
        agent: AgentKind::ClaudeCode,
        path: path.display().to_string(),
        has_tool_results: true,
        summary: summarize_chat(&messages),
        messages,
    })
}

fn parse_transcript(contents: &str) -> Vec<ChatMessage> {
    let mut messages: Vec<ChatMessage> = Vec::new();
    // Where each tool call is: its message's and its part's index.
    let mut tool_uses: HashMap<String, (usize, usize)> = HashMap::new();

    for line in contents.lines() {
        // The last line can be half written.
        let Ok(line) = serde_json::from_str::<TranscriptLine>(line) else {
            continue;
        };
        if line.is_sidechain {
            continue;
        }
        let role = match line.kind.as_str() {
            "user" => ChatRole::User,
            "assistant" if !line.is_meta => ChatRole::Assistant,
            _ => continue,
        };
        let Some(message) = &line.message else {
            continue;
        };
        if role == ChatRole::User {
            let text = content_text(&message.content);
            if let Some(kind) = note_kind(&line, &text) {
                let id = line
                    .uuid
                    .clone()
                    .unwrap_or_else(|| format!("line-{}", messages.len()));
                messages.extend(note_message(id, line.timestamp.clone(), kind, &text));
                continue;
            }
        }

        let mut parts = Vec::new();
        for part in read_parts(&message.content) {
            match part {
                ChatPart::ToolResult {
                    tool_use_id,
                    result,
                } => {
                    let tool_use = tool_use_id
                        .as_ref()
                        .and_then(|id| tool_uses.get(id))
                        .and_then(|&(message, part)| {
                            messages.get_mut(message)?.parts.get_mut(part)
                        });
                    match tool_use {
                        Some(ChatPart::ToolUse { result: slot, .. }) => *slot = Some(result),
                        _ => parts.push(ChatPart::ToolResult {
                            tool_use_id,
                            result,
                        }),
                    }
                }
                part => parts.push(part),
            }
        }
        if parts.is_empty() {
            continue;
        }

        // An assistant message is written one content block per line.
        let continues_last = role == ChatRole::Assistant
            && message.id.is_some()
            && messages
                .last()
                .is_some_and(|last| last.role == role && Some(&last.id) == message.id.as_ref());
        if !continues_last {
            let id = match role {
                ChatRole::Assistant => message.id.clone().or(line.uuid),
                _ => line.uuid,
            };
            messages.push(ChatMessage {
                id: id.unwrap_or_else(|| format!("line-{}", messages.len())),
                role,
                timestamp: line.timestamp,
                note: None,
                parts: Vec::new(),
            });
        }
        let message_index = messages.len() - 1;
        let Some(last) = messages.last_mut() else {
            continue;
        };
        for part in parts {
            if let ChatPart::ToolUse { id, .. } = &part {
                tool_uses.insert(id.clone(), (message_index, last.parts.len()));
            }
            last.parts.push(part);
        }
    }
    make_ids_unique(&mut messages);
    messages
}

/// Suffixes repeated ids with `-2`, `-3`, and so on. A note Claude Code
/// writes between two lines of one assistant message, like an image's size
/// after a `Read`, splits it into two messages with the message's id.
fn make_ids_unique(messages: &mut [ChatMessage]) {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for message in messages {
        let count = counts.entry(message.id.clone()).or_default();
        *count += 1;
        if *count > 1 {
            message.id = format!("{}-{count}", message.id);
        }
    }
}

/// Whether a user line is a note Claude Code wrote, and which kind. `None`
/// for a prompt the user typed and for tool results. Lines from older
/// versions, written without `origin`, are told apart by how they start.
fn note_kind(line: &TranscriptLine, text: &str) -> Option<ChatNoteKind> {
    if line.is_compact_summary {
        return Some(ChatNoteKind::CompactSummary);
    }
    if line.is_meta {
        return Some(ChatNoteKind::Context);
    }
    match line.origin.as_ref().map(|origin| origin.kind.as_str()) {
        Some("human") => return None,
        Some("task-notification") => return Some(ChatNoteKind::TaskNotification),
        Some(_) => return Some(ChatNoteKind::Other),
        None => {}
    }
    let text = text.trim_start();
    if text.is_empty() {
        return None;
    }
    let kind = if text.starts_with("<command-name>") || text.starts_with('/') {
        ChatNoteKind::Command
    } else if text.starts_with("<local-command-") {
        ChatNoteKind::CommandOutput
    } else if text.starts_with("<task-notification>") {
        ChatNoteKind::TaskNotification
    } else if text.starts_with("[Request interrupted") {
        ChatNoteKind::Interrupted
    } else {
        ChatNoteKind::Other
    };
    Some(kind)
}

fn note_message(
    id: String,
    timestamp: Option<String>,
    kind: ChatNoteKind,
    text: &str,
) -> Option<ChatMessage> {
    let text = strip_terminal_styles(text.trim());
    if text.is_empty() {
        return None;
    }
    let part = match kind {
        ChatNoteKind::CompactSummary => ChatPart::Text {
            html: render_markdown_html(&text),
        },
        _ => ChatPart::PlainText {
            text: tool_text(text.clone()),
        },
    };
    Some(ChatMessage {
        id,
        role: ChatRole::Note,
        timestamp,
        note: Some(ChatNote {
            kind,
            summary: note_summary(kind, &text),
        }),
        parts: vec![part],
    })
}

/// One line saying what a note holds: a command with its arguments, a task's
/// outcome, or else its first line without tags.
fn note_summary(kind: ChatNoteKind, text: &str) -> String {
    let summary = match kind {
        ChatNoteKind::Command => tag_contents(text, "command-name").map(|name| {
            let arguments = tag_contents(text, "command-args").unwrap_or_default();
            format!("{name} {arguments}")
        }),
        ChatNoteKind::TaskNotification => {
            tag_contents(text, "summary").or_else(|| tag_contents(text, "status"))
        }
        _ => None,
    };
    let summary = summary.unwrap_or_else(|| strip_tags(text));
    let first_line = summary
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_default();
    shorten(first_line, MAX_TOOL_SUMMARY_CHARS)
}

/// Takes out terminal colour and style codes, like `\x1b[2m`, that command
/// output keeps.
fn strip_terminal_styles(text: &str) -> String {
    let mut stripped = String::with_capacity(text.len());
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        if character == '\u{1b}' && characters.peek() == Some(&'[') {
            characters.next();
            for code in characters.by_ref() {
                if code.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            stripped.push(character);
        }
    }
    stripped
}

/// The parts of a message's `content`: a string, or a list of blocks.
fn read_parts(content: &Value) -> Vec<ChatPart> {
    match content {
        Value::String(text) => text_part(text).into_iter().collect(),
        Value::Array(blocks) => blocks.iter().filter_map(read_block).collect(),
        _ => Vec::new(),
    }
}

fn read_block(block: &Value) -> Option<ChatPart> {
    match block.get("type")?.as_str()? {
        "text" => text_part(block.get("text")?.as_str()?),
        "thinking" => {
            let text = block.get("thinking")?.as_str()?.trim();
            // Thinking is often written empty, with only its signature.
            (!text.is_empty()).then(|| ChatPart::Thinking {
                text: text.to_string(),
            })
        }
        "tool_use" => {
            let input = block.get("input").unwrap_or(&Value::Null);
            Some(ChatPart::ToolUse {
                id: block.get("id")?.as_str()?.to_string(),
                name: block.get("name")?.as_str()?.to_string(),
                summary: tool_summary(input),
                input: tool_text(serde_json::to_string_pretty(input).unwrap_or_default()),
                result: None,
            })
        }
        "tool_result" => Some(ChatPart::ToolResult {
            tool_use_id: block
                .get("tool_use_id")
                .and_then(Value::as_str)
                .map(str::to_string),
            result: ToolResult {
                output: tool_text(tool_result_text(block.get("content"))),
                is_error: block
                    .get("is_error")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            },
        }),
        _ => None,
    }
}

/// A result's content is a string, or a list of text and image blocks.
fn tool_result_text(content: Option<&Value>) -> String {
    match content {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Array(blocks)) => blocks
            .iter()
            .map(|block| match block.get("type").and_then(Value::as_str) {
                Some("text") => block
                    .get("text")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                Some(other) => format!("[{other}]"),
                None => String::new(),
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_chat::ToolCallCount;

    const TRANSCRIPT: &str = r#"{"type":"mode","mode":"normal","sessionId":"s1"}
{"type":"user","uuid":"u1","timestamp":"2026-11-04T19:50:08.854Z","isSidechain":false,"origin":{"kind":"human"},"message":{"role":"user","content":"Fix the **bug**"}}
{"type":"user","uuid":"u0","isMeta":true,"message":{"role":"user","content":"<system-reminder>context</system-reminder>"}}
{"type":"assistant","uuid":"a1","message":{"id":"msg_1","role":"assistant","content":[{"type":"thinking","thinking":"","signature":"x"}]}}
{"type":"assistant","uuid":"a2","message":{"id":"msg_1","role":"assistant","content":[{"type":"text","text":"Looking."}]}}
{"type":"assistant","uuid":"a3","message":{"id":"msg_1","role":"assistant","content":[{"type":"tool_use","id":"toolu_1","name":"Bash","input":{"command":"ls"}}]}}
{"type":"user","uuid":"u2","message":{"role":"user","content":[{"tool_use_id":"toolu_1","type":"tool_result","content":"Cargo.toml","is_error":false}]}}
{"type":"user","uuid":"u3","isSidechain":true,"message":{"role":"user","content":"a subagent's prompt"}}
{"type":"assistant","uuid":"a4","message":{"id":"msg_2","role":"assistant","content":[{"type":"text","text":"Done"#;

    #[test]
    fn reads_the_users_and_the_assistants_messages() {
        let messages = parse_transcript(TRANSCRIPT);

        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].role, ChatRole::User);
        assert_eq!(messages[0].id, "u1");
        assert_eq!(
            messages[0].parts,
            vec![ChatPart::Text {
                html: render_markdown_html("Fix the **bug**")
            }]
        );
        assert_eq!(messages[1].role, ChatRole::Note);
        assert_eq!(messages[2].role, ChatRole::Assistant);
        assert_eq!(messages[2].id, "msg_1");
    }

    const NOTES: &str = r#"{"type":"user","uuid":"n1","isMeta":true,"message":{"role":"user","content":"<system-reminder>\nRead AGENTS.md\n</system-reminder>"}}
{"type":"user","uuid":"n2","isCompactSummary":true,"isVisibleInTranscriptOnly":true,"message":{"role":"user","content":"This session is being **continued**."}}
{"type":"user","uuid":"n3","message":{"role":"user","content":"<command-name>/model</command-name>\n<command-message>model</command-message>\n<command-args>opus</command-args>"}}
{"type":"user","uuid":"n4","message":{"role":"user","content":"<local-command-stdout>\u001b[2mCompacted\u001b[22m</local-command-stdout>"}}
{"type":"user","uuid":"n5","origin":{"kind":"task-notification"},"message":{"role":"user","content":"<task-notification>\n<status>completed</status>\n<summary>Background command finished</summary>\n</task-notification>"}}
{"type":"user","uuid":"n6","message":{"role":"user","content":[{"type":"text","text":"[Request interrupted by user]"}]}}
{"type":"user","uuid":"n7","origin":{"kind":"human"},"promptSource":"typed","message":{"role":"user","content":"<pasted>a prompt</pasted>"}}"#;

    #[test]
    fn keeps_the_lines_claude_code_writes_as_notes() {
        let messages = parse_transcript(NOTES);

        let notes: Vec<_> = messages
            .iter()
            .filter_map(|message| message.note.as_ref())
            .map(|note| (note.kind, note.summary.as_str()))
            .collect();
        assert_eq!(
            notes,
            vec![
                (ChatNoteKind::Context, "Read AGENTS.md"),
                (
                    ChatNoteKind::CompactSummary,
                    "This session is being **continued**."
                ),
                (ChatNoteKind::Command, "/model opus"),
                (ChatNoteKind::CommandOutput, "Compacted"),
                (
                    ChatNoteKind::TaskNotification,
                    "Background command finished"
                ),
                (ChatNoteKind::Interrupted, "[Request interrupted by user]"),
            ]
        );
        assert_eq!(messages[6].role, ChatRole::User);
        assert!(messages[6].is_prompt());
        assert_eq!(summarize_chat(&messages).prompts, 1);
        assert_eq!(summarize_chat(&messages).notes, 6);
    }

    const SPLIT_BY_A_NOTE: &str = r#"{"type":"assistant","uuid":"a1","message":{"id":"msg_1","role":"assistant","content":[{"type":"tool_use","id":"toolu_1","name":"Read","input":{"file_path":"a.png"}}]}}
{"type":"user","uuid":"u1","message":{"role":"user","content":[{"tool_use_id":"toolu_1","type":"tool_result","content":"image"}]}}
{"type":"user","uuid":"n1","isMeta":true,"message":{"role":"user","content":"[Image: original 1170x2532]"}}
{"type":"assistant","uuid":"a2","message":{"id":"msg_1","role":"assistant","content":[{"type":"tool_use","id":"toolu_2","name":"Read","input":{"file_path":"b.png"}}]}}
{"type":"assistant","uuid":"a3","message":{"id":"msg_1","role":"assistant","content":[{"type":"text","text":"Both look right."}]}}"#;

    #[test]
    fn gives_an_assistant_message_split_by_a_note_unique_ids() {
        let messages = parse_transcript(SPLIT_BY_A_NOTE);

        let ids: Vec<_> = messages.iter().map(|message| message.id.as_str()).collect();
        assert_eq!(ids, vec!["msg_1", "n1", "msg_1-2"]);
        assert_eq!(messages[2].parts.len(), 2);
    }

    #[test]
    fn pairs_tool_calls_with_their_results() {
        let messages = parse_transcript(TRANSCRIPT);

        let [
            ChatPart::Text { .. },
            ChatPart::ToolUse {
                name,
                summary,
                input,
                result,
                ..
            },
        ] = messages[2].parts.as_slice()
        else {
            panic!(
                "expected a text and a tool call, got {:?}",
                messages[2].parts
            );
        };
        assert_eq!(name, "Bash");
        assert_eq!(summary, "ls");
        assert!(input.text.contains("\"command\": \"ls\""));
        let result = result.as_ref().expect("the result is paired");
        assert_eq!(result.output.text, "Cargo.toml");
        assert!(!result.is_error);
    }

    #[test]
    fn summarizes_the_chat() {
        let summary = summarize_chat(&parse_transcript(TRANSCRIPT));

        assert_eq!(summary.prompts, 1);
        assert_eq!(summary.assistant_messages, 1);
        assert_eq!(summary.tool_calls, 1);
        assert_eq!(summary.failed_tool_calls, 0);
        assert_eq!(summary.tool_calls_without_result, 0);
        assert_eq!(
            summary.tool_calls_by_name,
            vec![ToolCallCount {
                name: "Bash".into(),
                count: 1
            }]
        );
        assert_eq!(
            summary.started_at.as_deref(),
            Some("2026-11-04T19:50:08.854Z")
        );
    }
}
