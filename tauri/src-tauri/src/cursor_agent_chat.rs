//! Reads the chat of Cursor's `agent` from the transcript it writes to
//! `~/.cursor/projects/<directory>/agent-transcripts/<chat id>/<chat id>.jsonl`,
//! one `{"role", "message": {"content"}}` object per line.
//!
//! The transcript is undocumented and holds less than Claude Code's: the
//! tool calls without their results, no ids, and a time only on the user's
//! prompts, written into the prompt's text. Lines that don't parse are
//! skipped. The chat database beside it, `~/.cursor/chats/…/store.db`, has
//! the results, but in a binary format, so it isn't read.

use std::path::Path;

use chrono::{FixedOffset, NaiveDateTime, TimeZone, Utc};
use serde::Deserialize;
use serde_json::Value;

use crate::agent::AgentKind;
use crate::agent_chat::{
    ChatHistory, ChatMessage, ChatNote, ChatNoteKind, ChatPart, ChatRole, MAX_TOOL_SUMMARY_CHARS,
    ToolText, content_text, find_in_subdirectories, home_directory_path, shorten, strip_tags,
    summarize_chat, tag_contents, text_part, tool_summary, tool_text, validate_chat_id,
};

/// How `agent` writes a prompt's time, like `Thursday, Nov 5, 2026, 4:12 PM`,
/// followed by ` (UTC-5)`.
const PROMPT_TIME_FORMAT: &str = "%A, %b %d, %Y, %I:%M %p";

#[derive(Deserialize)]
struct TranscriptLine {
    role: String,
    message: Option<TranscriptMessage>,
}

#[derive(Deserialize)]
struct TranscriptMessage {
    content: Value,
}

/// The chat `chat_id` of Cursor's `agent`. Reads the whole transcript, so
/// call it from a blocking task.
pub fn read_chat_history(chat_id: &str) -> Result<ChatHistory, String> {
    validate_chat_id(chat_id)?;
    let projects = home_directory_path(".cursor/projects").ok_or("HOME is not set")?;
    let transcript = Path::new("agent-transcripts")
        .join(chat_id)
        .join(format!("{chat_id}.jsonl"));
    let path = find_in_subdirectories(&projects, &transcript)
        .ok_or_else(|| format!("No transcript for Cursor chat {chat_id}"))?;
    let contents = std::fs::read_to_string(&path)
        .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
    let messages = parse_transcript(&contents);
    Ok(ChatHistory {
        agent: AgentKind::CursorAgent,
        path: path.display().to_string(),
        has_tool_results: false,
        summary: summarize_chat(&messages),
        messages,
    })
}

fn parse_transcript(contents: &str) -> Vec<ChatMessage> {
    contents
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            // The last line can be half written.
            let line = serde_json::from_str::<TranscriptLine>(line).ok()?;
            let content = line.message?.content;
            let id = format!("line-{index}");
            match line.role.as_str() {
                "user" => read_user_message(id, &content_text(&content)),
                "assistant" => read_assistant_message(id, &content),
                _ => None,
            }
        })
        .collect()
}

/// A prompt, its text in `<user_query>` after its time in `<timestamp>`;
/// anything else as a note.
fn read_user_message(id: String, text: &str) -> Option<ChatMessage> {
    let Some(query) = tag_contents(text, "user_query") else {
        let text = text.trim();
        let summary = strip_tags(text);
        let first_line = summary
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())?;
        return Some(ChatMessage {
            id,
            role: ChatRole::Note,
            timestamp: None,
            note: Some(ChatNote {
                kind: ChatNoteKind::Other,
                summary: shorten(first_line, MAX_TOOL_SUMMARY_CHARS),
            }),
            parts: vec![ChatPart::PlainText {
                text: tool_text(text.to_string()),
            }],
        });
    };
    Some(ChatMessage {
        id,
        role: ChatRole::User,
        timestamp: tag_contents(text, "timestamp").and_then(|time| parse_prompt_time(&time)),
        note: None,
        parts: vec![text_part(&query)?],
    })
}

fn read_assistant_message(id: String, content: &Value) -> Option<ChatMessage> {
    let blocks = content.as_array()?;
    let parts: Vec<ChatPart> = blocks
        .iter()
        .enumerate()
        .filter_map(|(index, block)| read_block(&format!("{id}-{index}"), block))
        .collect();
    (!parts.is_empty()).then_some(ChatMessage {
        id,
        role: ChatRole::Assistant,
        timestamp: None,
        note: None,
        parts,
    })
}

fn read_block(id: &str, block: &Value) -> Option<ChatPart> {
    match block.get("type")?.as_str()? {
        "text" => text_part(block.get("text")?.as_str()?),
        "tool_use" => {
            let input = block.get("input").unwrap_or(&Value::Null);
            Some(ChatPart::ToolUse {
                id: id.to_string(),
                name: block.get("name")?.as_str()?.to_string(),
                summary: tool_summary(input),
                input: input_text(input),
                result: None,
            })
        }
        _ => None,
    }
}

fn input_text(input: &Value) -> ToolText {
    tool_text(serde_json::to_string_pretty(input).unwrap_or_default())
}

/// `Thursday, Nov 5, 2026, 4:12 PM (UTC-5)` as `2026-11-05T21:12:00.000Z`,
/// like Claude Code's timestamps.
fn parse_prompt_time(time: &str) -> Option<String> {
    let (local, offset) = time.trim().split_once(" (UTC")?;
    let local = NaiveDateTime::parse_from_str(local, PROMPT_TIME_FORMAT).ok()?;
    let offset = parse_utc_offset(offset.strip_suffix(')')?)?;
    let time = offset.from_local_datetime(&local).single()?;
    Some(
        time.with_timezone(&Utc)
            .format("%Y-%m-%dT%H:%M:%S%.3fZ")
            .to_string(),
    )
}

/// `-5`, `+5:30` or nothing, for UTC itself.
fn parse_utc_offset(offset: &str) -> Option<FixedOffset> {
    if offset.is_empty() {
        return FixedOffset::east_opt(0);
    }
    let (sign, rest) = match offset.split_at_checked(1)? {
        ("+", rest) => (1, rest),
        ("-", rest) => (-1, rest),
        _ => return None,
    };
    let (hours, minutes) = rest.split_once(':').unwrap_or((rest, "0"));
    let seconds = hours.parse::<i32>().ok()? * 3600 + minutes.parse::<i32>().ok()? * 60;
    FixedOffset::east_opt(sign * seconds)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TRANSCRIPT: &str = r#"{"role":"user","message":{"content":[{"type":"text","text":"<timestamp>Thursday, Nov 5, 2026, 4:12 PM (UTC-5)</timestamp>\n<user_query>\nhey cursor, are you there?\n</user_query>"}]}}
{"role":"assistant","message":{"content":[{"type":"text","text":"Yes, I'm here."}]}}
{"role":"assistant","message":{"content":[{"type":"text","text":"Looking."},{"type":"tool_use","name":"Read","input":{"path":"/repo/HomePage.svelte"}}]}}
{"role":"user","message":{"content":[{"type":"text","text":"<attached_files>\na.rs\n</attached_files>"}]}}
{"role":"assistant","message":{"content":[{"type":"text","text":"Do"#;

    #[test]
    fn reads_prompts_replies_and_tool_calls() {
        let messages = parse_transcript(TRANSCRIPT);

        assert_eq!(messages.len(), 4);
        assert_eq!(messages[0].role, ChatRole::User);
        assert_eq!(
            messages[0].parts,
            vec![ChatPart::Text {
                html: code_reviewer_core::analysis::markdown_html::render_markdown_html(
                    "hey cursor, are you there?"
                )
            }]
        );
        assert_eq!(
            messages[0].timestamp.as_deref(),
            Some("2026-11-05T21:12:00.000Z")
        );
        assert_eq!(messages[1].role, ChatRole::Assistant);
        assert_eq!(messages[1].timestamp, None);
        let [
            ChatPart::Text { .. },
            ChatPart::ToolUse {
                name,
                summary,
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
        assert_eq!(name, "Read");
        assert_eq!(summary, "/repo/HomePage.svelte");
        assert_eq!(result, &None);
        assert_eq!(messages[3].role, ChatRole::Note);
        assert_eq!(
            messages[3].note.as_ref().map(|note| note.summary.as_str()),
            Some("a.rs")
        );
    }

    #[test]
    fn reads_prompt_times_in_any_offset() {
        assert_eq!(
            parse_prompt_time("Sunday, Jan 18, 2026, 12:05 AM (UTC+5:30)").as_deref(),
            Some("2026-01-17T18:35:00.000Z")
        );
        assert_eq!(
            parse_prompt_time("Thursday, Nov 5, 2026, 4:12 PM (UTC)").as_deref(),
            Some("2026-11-05T16:12:00.000Z")
        );
        assert_eq!(parse_prompt_time("yesterday"), None);
    }
}
