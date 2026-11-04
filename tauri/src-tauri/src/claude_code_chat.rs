//! Reads a Claude Code session's chat from the transcript Claude Code writes
//! to `~/.claude/projects/<directory>/<session id>.jsonl`, one JSON object
//! per line.
//!
//! The transcript is undocumented and may change between Claude Code
//! versions, so lines that don't parse, and kinds of lines or content the
//! chat doesn't show, are skipped. Only the user's and the assistant's
//! messages are kept, with each tool call paired with its result.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use code_reviewer_core::analysis::markdown_html::render_markdown_html;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Longer tool inputs and results are cut to this many characters, so a
/// large file read doesn't fill the page.
const MAX_TOOL_TEXT_CHARS: usize = 20_000;
/// The input fields that best say what a tool call does, in order.
const TOOL_SUMMARY_FIELDS: [&str; 8] = [
    "description",
    "command",
    "file_path",
    "pattern",
    "path",
    "url",
    "query",
    "prompt",
];
const MAX_TOOL_SUMMARY_CHARS: usize = 160;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ChatHistory {
    /// The transcript the chat was read from.
    pub path: String,
    pub messages: Vec<ChatMessage>,
    pub summary: ChatSummary,
}

/// Counts over the whole chat, whatever the page filters.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct ChatSummary {
    /// The messages the user typed (see `ChatMessage::is_prompt`).
    pub prompts: usize,
    pub assistant_messages: usize,
    pub thinking_blocks: usize,
    pub tool_calls: usize,
    pub failed_tool_calls: usize,
    /// Tool calls whose result isn't written yet, like one still running.
    pub tool_calls_without_result: usize,
    /// Each tool's name with how many times it was called, most called first.
    pub tool_calls_by_name: Vec<ToolCallCount>,
    /// The first message's timestamp.
    pub started_at: Option<String>,
    /// The last message's timestamp.
    pub last_message_at: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ToolCallCount {
    pub name: String,
    pub count: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ChatMessage {
    /// The transcript line's `uuid`, or the API message id when an assistant
    /// message spans several lines.
    pub id: String,
    pub role: ChatRole,
    /// Like `2026-11-04T19:50:08.854Z`.
    pub timestamp: Option<String>,
    pub parts: Vec<ChatPart>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatRole {
    User,
    Assistant,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChatPart {
    /// Markdown, rendered as HTML with raw HTML left out.
    Text {
        html: String,
    },
    Thinking {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        /// One line from the input saying what the call does, like the
        /// command it runs or the file it reads. Empty when none says.
        summary: String,
        /// The input as pretty-printed JSON.
        input: ToolText,
        result: Option<ToolResult>,
    },
    /// A result whose tool call isn't in the transcript.
    ToolResult {
        tool_use_id: Option<String>,
        result: ToolResult,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ToolResult {
    pub output: ToolText,
    pub is_error: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ToolText {
    pub text: String,
    /// Whether `text` was cut to `MAX_TOOL_TEXT_CHARS`.
    pub truncated: bool,
}

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
}

#[derive(Deserialize)]
struct TranscriptMessage {
    id: Option<String>,
    content: Value,
}

/// The chat of the Claude Code session `session_id`. Reads the whole
/// transcript, so call it from a blocking task.
pub fn read_chat_history(session_id: &str) -> Result<ChatHistory, String> {
    validate_session_id(session_id)?;
    let projects = projects_directory().ok_or("HOME is not set")?;
    let path = find_transcript(&projects, session_id)
        .ok_or_else(|| format!("No transcript for Claude Code session {session_id}"))?;
    let contents = std::fs::read_to_string(&path)
        .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
    let messages = parse_transcript(&contents);
    Ok(ChatHistory {
        path: path.display().to_string(),
        summary: summarize_chat(&messages),
        messages,
    })
}

impl ChatMessage {
    /// A message the user typed, not only tool results passed back.
    pub fn is_prompt(&self) -> bool {
        self.role == ChatRole::User
            && self
                .parts
                .iter()
                .any(|part| matches!(part, ChatPart::Text { .. }))
    }
}

fn summarize_chat(messages: &[ChatMessage]) -> ChatSummary {
    let mut summary = ChatSummary {
        prompts: messages
            .iter()
            .filter(|message| message.is_prompt())
            .count(),
        assistant_messages: messages
            .iter()
            .filter(|message| message.role == ChatRole::Assistant)
            .count(),
        started_at: messages
            .first()
            .and_then(|message| message.timestamp.clone()),
        last_message_at: messages
            .last()
            .and_then(|message| message.timestamp.clone()),
        ..ChatSummary::default()
    };
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for part in messages.iter().flat_map(|message| &message.parts) {
        match part {
            ChatPart::Thinking { .. } => summary.thinking_blocks += 1,
            ChatPart::ToolUse { name, result, .. } => {
                summary.tool_calls += 1;
                *counts.entry(name).or_default() += 1;
                match result {
                    Some(result) if result.is_error => summary.failed_tool_calls += 1,
                    Some(_) => {}
                    None => summary.tool_calls_without_result += 1,
                }
            }
            _ => {}
        }
    }
    summary.tool_calls_by_name = counts
        .into_iter()
        .map(|(name, count)| ToolCallCount {
            name: name.to_string(),
            count,
        })
        .collect();
    summary
        .tool_calls_by_name
        .sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.name.cmp(&b.name)));
    summary
}

/// Session ids are UUIDs; anything else could reach outside the directory.
fn validate_session_id(session_id: &str) -> Result<(), String> {
    let is_valid = !session_id.is_empty()
        && session_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-');
    if is_valid {
        Ok(())
    } else {
        Err(format!("{session_id} is not a Claude Code session id"))
    }
}

fn projects_directory() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    Some(PathBuf::from(home).join(".claude/projects"))
}

/// Looks in every project directory rather than spelling out the
/// directory's name from the session's, which Claude Code doesn't document.
fn find_transcript(projects: &Path, session_id: &str) -> Option<PathBuf> {
    let file_name = format!("{session_id}.jsonl");
    std::fs::read_dir(projects)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path().join(&file_name))
        .find(|path| path.is_file())
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
        if line.is_sidechain || line.is_meta {
            continue;
        }
        let role = match line.kind.as_str() {
            "user" => ChatRole::User,
            "assistant" => ChatRole::Assistant,
            _ => continue,
        };
        let Some(message) = line.message else {
            continue;
        };

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
                ChatRole::Assistant => message.id.or(line.uuid),
                ChatRole::User => line.uuid,
            };
            messages.push(ChatMessage {
                id: id.unwrap_or_else(|| format!("line-{}", messages.len())),
                role,
                timestamp: line.timestamp,
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
    messages
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

fn tool_summary(input: &Value) -> String {
    let first_line = TOOL_SUMMARY_FIELDS
        .iter()
        .filter_map(|field| input.get(field)?.as_str())
        .find_map(|value| value.lines().map(str::trim).find(|line| !line.is_empty()))
        .unwrap_or_default();
    match first_line.char_indices().nth(MAX_TOOL_SUMMARY_CHARS) {
        Some((cut, _)) => format!("{}…", &first_line[..cut]),
        None => first_line.to_string(),
    }
}

fn text_part(text: &str) -> Option<ChatPart> {
    let text = text.trim();
    (!text.is_empty()).then(|| ChatPart::Text {
        html: render_markdown_html(text),
    })
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

fn tool_text(text: String) -> ToolText {
    match text.char_indices().nth(MAX_TOOL_TEXT_CHARS) {
        Some((cut, _)) => ToolText {
            text: text[..cut].to_string(),
            truncated: true,
        },
        None => ToolText {
            text,
            truncated: false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TRANSCRIPT: &str = r#"{"type":"mode","mode":"normal","sessionId":"s1"}
{"type":"user","uuid":"u1","timestamp":"2026-11-04T19:50:08.854Z","isSidechain":false,"message":{"role":"user","content":"Fix the **bug**"}}
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

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, ChatRole::User);
        assert_eq!(messages[0].id, "u1");
        assert_eq!(
            messages[0].parts,
            vec![ChatPart::Text {
                html: render_markdown_html("Fix the **bug**")
            }]
        );
        assert_eq!(messages[1].role, ChatRole::Assistant);
        assert_eq!(messages[1].id, "msg_1");
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
        ] = messages[1].parts.as_slice()
        else {
            panic!(
                "expected a text and a tool call, got {:?}",
                messages[1].parts
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

    #[test]
    fn cuts_long_tool_text() {
        let text = tool_text("é".repeat(MAX_TOOL_TEXT_CHARS + 5));
        assert!(text.truncated);
        assert_eq!(text.text.chars().count(), MAX_TOOL_TEXT_CHARS);
    }

    #[test]
    fn refuses_session_ids_that_are_paths() {
        assert!(validate_session_id("03d0ee05-e0c2-403a-8b99-c7ce5d5623b1").is_ok());
        assert!(validate_session_id("../secrets").is_err());
        assert!(validate_session_id("").is_err());
    }
}
