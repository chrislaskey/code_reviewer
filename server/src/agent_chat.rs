//! The chat of a coding agent, as the session page shows it, whichever
//! agent wrote it: Claude Code (`claude_code_chat.rs`) or Cursor's `agent`
//! (`cursor_agent_chat.rs`). Also the helpers both readers share.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use code_reviewer_core::analysis::markdown_html::render_markdown_html;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::agent::AgentKind;

/// Longer tool inputs and results are cut to this many characters, so a
/// large file read doesn't fill the page.
pub(crate) const MAX_TOOL_TEXT_CHARS: usize = 20_000;
/// The input fields that best say what a tool call does, in order.
pub(crate) const TOOL_SUMMARY_FIELDS: [&str; 9] = [
    "description",
    "command",
    "file_path",
    "pattern",
    "glob_pattern",
    "path",
    "url",
    "query",
    "prompt",
];
pub(crate) const MAX_TOOL_SUMMARY_CHARS: usize = 160;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct ChatHistory {
    /// The agent that wrote the chat.
    pub agent: AgentKind,
    /// The transcript the chat was read from.
    pub path: String,
    /// Whether the transcript has tool results. Cursor's doesn't, so its
    /// tool calls have none and that doesn't mean they are still running.
    pub has_tool_results: bool,
    pub messages: Vec<ChatMessage>,
    pub summary: ChatSummary,
}

/// Counts over the whole chat, whatever the page filters.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct ChatSummary {
    /// The messages the user typed (see `ChatMessage::is_prompt`).
    pub prompts: usize,
    pub assistant_messages: usize,
    /// Lines the agent's CLI wrote, like context and command output.
    pub notes: usize,
    pub thinking_blocks: usize,
    pub tool_calls: usize,
    pub failed_tool_calls: usize,
    /// Tool calls whose result isn't written yet, like one still running.
    pub tool_calls_without_result: usize,
    /// Each tool's name with how many times it was called, most called first.
    pub tool_calls_by_name: Vec<ToolCallCount>,
    /// The first message's timestamp. Cursor stamps only the prompts.
    pub started_at: Option<String>,
    /// The last message's timestamp, of those that have one.
    pub last_message_at: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct ToolCallCount {
    pub name: String,
    pub count: usize,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct ChatMessage {
    /// The transcript line's `uuid`, or the API message id when an assistant
    /// message spans several lines.
    pub id: String,
    pub role: ChatRole,
    /// Like `2026-11-04T19:50:08.854Z`.
    pub timestamp: Option<String>,
    /// What a `Note` message is; `None` for the other roles.
    pub note: Option<ChatNote>,
    pub parts: Vec<ChatPart>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatRole {
    User,
    Assistant,
    /// A line the agent's CLI wrote as the user's, not typed by the user.
    Note,
}

/// What a line the agent's CLI wrote is, with one line saying what it holds.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct ChatNote {
    pub kind: ChatNoteKind,
    /// Like the command run or the background task's outcome.
    pub summary: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatNoteKind {
    /// Context added for the agent, like system reminders and skills.
    Context,
    /// The summary that continues a conversation after it was compacted.
    CompactSummary,
    /// A slash command the user ran, like `/compact`.
    Command,
    /// What a slash command printed.
    CommandOutput,
    /// A background task finished.
    TaskNotification,
    /// The user stopped the agent.
    Interrupted,
    Other,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChatPart {
    /// Markdown, rendered as HTML with raw HTML left out.
    Text {
        html: String,
    },
    /// Text shown as written, as a note's is, since it is mostly tags.
    PlainText {
        text: ToolText,
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

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct ToolResult {
    pub output: ToolText,
    pub is_error: bool,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct ToolText {
    pub text: String,
    /// Whether `text` was cut to `MAX_TOOL_TEXT_CHARS`.
    pub truncated: bool,
}

impl ChatMessage {
    /// A message the user typed, not tool results passed back or a note.
    pub fn is_prompt(&self) -> bool {
        self.role == ChatRole::User
            && self
                .parts
                .iter()
                .any(|part| matches!(part, ChatPart::Text { .. }))
    }
}

pub(crate) fn summarize_chat(messages: &[ChatMessage]) -> ChatSummary {
    let mut summary = ChatSummary {
        prompts: messages
            .iter()
            .filter(|message| message.is_prompt())
            .count(),
        assistant_messages: messages
            .iter()
            .filter(|message| message.role == ChatRole::Assistant)
            .count(),
        notes: messages
            .iter()
            .filter(|message| message.role == ChatRole::Note)
            .count(),
        started_at: messages
            .iter()
            .find_map(|message| message.timestamp.clone()),
        last_message_at: messages
            .iter()
            .rev()
            .find_map(|message| message.timestamp.clone()),
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

/// Chat ids are UUIDs; anything else could reach outside the directory.
pub(crate) fn validate_chat_id(chat_id: &str) -> Result<(), String> {
    let is_valid = !chat_id.is_empty()
        && chat_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-');
    if is_valid {
        Ok(())
    } else {
        Err(format!("{chat_id} is not a chat id"))
    }
}

/// The `file_name` in any directory of `directory`, looked for in each
/// rather than spelling out the directory's name, which the agents don't
/// document.
pub(crate) fn find_in_subdirectories(directory: &Path, file_name: &Path) -> Option<PathBuf> {
    std::fs::read_dir(directory)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path().join(file_name))
        .find(|path| path.is_file())
}

/// `~/<path>`.
pub(crate) fn home_directory_path(path: &str) -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    Some(PathBuf::from(home).join(path))
}

/// What's between `<tag>` and `</tag>`, trimmed.
pub(crate) fn tag_contents(text: &str, tag: &str) -> Option<String> {
    let start = text.find(&format!("<{tag}>"))? + tag.len() + 2;
    let length = text[start..].find(&format!("</{tag}>"))?;
    Some(text[start..start + length].trim().to_string())
}

/// Takes out tags like `<command-name>` and `</user_query>`.
pub(crate) fn strip_tags(text: &str) -> String {
    let mut stripped = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('<') {
        stripped.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let name_length = after.trim_start_matches('/').find(|character: char| {
            !(character.is_ascii_alphanumeric() || "-_".contains(character))
        });
        let slash = usize::from(after.starts_with('/'));
        match name_length {
            Some(length) if length > 0 && after[slash + length..].starts_with('>') => {
                rest = &after[slash + length + 1..];
            }
            _ => {
                stripped.push('<');
                rest = after;
            }
        }
    }
    stripped.push_str(rest);
    stripped
}

/// The text of a message's `content`: a string, or its text blocks.
pub(crate) fn content_text(content: &Value) -> String {
    match content {
        Value::String(text) => text.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter(|block| block.get("type").and_then(Value::as_str) == Some("text"))
            .filter_map(|block| block.get("text")?.as_str())
            .collect::<Vec<_>>()
            .join("\n\n"),
        _ => String::new(),
    }
}

pub(crate) fn tool_summary(input: &Value) -> String {
    let first_line = TOOL_SUMMARY_FIELDS
        .iter()
        .filter_map(|field| input.get(field)?.as_str())
        .find_map(|value| value.lines().map(str::trim).find(|line| !line.is_empty()))
        .unwrap_or_default();
    shorten(first_line, MAX_TOOL_SUMMARY_CHARS)
}

pub(crate) fn shorten(text: &str, max_chars: usize) -> String {
    match text.char_indices().nth(max_chars) {
        Some((cut, _)) => format!("{}…", &text[..cut]),
        None => text.to_string(),
    }
}

pub(crate) fn text_part(text: &str) -> Option<ChatPart> {
    let text = text.trim();
    (!text.is_empty()).then(|| ChatPart::Text {
        html: render_markdown_html(text),
    })
}

pub(crate) fn tool_text(text: String) -> ToolText {
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

    #[test]
    fn cuts_long_tool_text() {
        let text = tool_text("é".repeat(MAX_TOOL_TEXT_CHARS + 5));
        assert!(text.truncated);
        assert_eq!(text.text.chars().count(), MAX_TOOL_TEXT_CHARS);
    }

    #[test]
    fn refuses_chat_ids_that_are_paths() {
        assert!(validate_chat_id("03d0ee05-e0c2-403a-8b99-c7ce5d5623b1").is_ok());
        assert!(validate_chat_id("../secrets").is_err());
        assert!(validate_chat_id("").is_err());
    }

    #[test]
    fn strips_tags_but_not_other_angle_brackets() {
        assert_eq!(strip_tags("<a-b>x</a_b> 1 < 2 <=> <"), "x 1 < 2 <=> <");
    }
}
