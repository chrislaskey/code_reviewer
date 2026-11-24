//! The command Continue types into a worktree's new session: the agent that
//! last chatted in the worktree, resuming its chat, else Claude Code's
//! `claude --continue`. Each agent's chats are found where it keeps them
//! for a directory:
//!
//! * Claude Code: `~/.claude/projects/<directory, each non-alphanumeric as ->/`
//! * Cursor's `agent`: `~/.cursor/chats/<MD5 of the directory>/<chat id>/`
//! * Codex: the rollouts whose first line, `session_meta`, has it as `cwd`

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use md5::{Digest, Md5};
use serde_json::Value;

use crate::services::agents::chat::{home_directory_path, validate_chat_id};
use crate::services::agents::codex;
use crate::services::agents::codex_chat::list_subdirectories_newest_first;

const CLAUDE_CODE_CONTINUE_COMMAND: &str = "claude --continue";

#[derive(Debug, PartialEq)]
enum LastChat {
    ClaudeCode,
    CursorAgent { chat_id: String },
    Codex { session_id: String },
}

struct FoundChat {
    chat: LastChat,
    last_write: SystemTime,
}

/// The command that resumes the chat last written to in `directory`.
pub fn find_continue_command(directory: &Path) -> String {
    let found_chats = [
        home_directory_path(".claude/projects")
            .and_then(|projects| find_claude_code_chat(&projects, directory)),
        home_directory_path(".cursor/chats")
            .and_then(|chats| find_cursor_agent_chat(&chats, directory)),
        codex::sessions_directory().and_then(|sessions| find_codex_chat(&sessions, directory)),
    ];
    let last_chat = found_chats
        .into_iter()
        .flatten()
        .max_by_key(|found| found.last_write)
        .map(|found| found.chat);
    continue_command(last_chat)
}

fn continue_command(last_chat: Option<LastChat>) -> String {
    match last_chat {
        Some(LastChat::CursorAgent { chat_id }) => format!("cursor-agent --resume {chat_id}"),
        Some(LastChat::Codex { session_id }) => format!("codex resume {session_id}"),
        Some(LastChat::ClaudeCode) | None => CLAUDE_CODE_CONTINUE_COMMAND.to_string(),
    }
}

fn find_claude_code_chat(projects: &Path, directory: &Path) -> Option<FoundChat> {
    let project = projects.join(claude_code_project_name(directory.to_str()?));
    let last_write = list_files(&project)
        .into_iter()
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "jsonl")
        })
        .filter_map(|path| last_write_of(&path))
        .max()?;
    Some(FoundChat {
        chat: LastChat::ClaudeCode,
        last_write,
    })
}

/// `/Users/me/code/app/.worktrees/cr-x` → `-Users-me-code-app--worktrees-cr-x`.
fn claude_code_project_name(directory: &str) -> String {
    directory
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect()
}

fn find_cursor_agent_chat(chats: &Path, directory: &Path) -> Option<FoundChat> {
    let workspace = chats.join(cursor_agent_workspace_name(directory.to_str()?));
    list_subdirectories_newest_first(&workspace)
        .into_iter()
        .filter_map(|chat| {
            let chat_id = chat.file_name()?.to_str()?.to_string();
            validate_chat_id(&chat_id).ok()?;
            let last_write = list_files(&chat)
                .iter()
                .filter_map(|path| last_write_of(path))
                .max()?;
            Some(FoundChat {
                chat: LastChat::CursorAgent { chat_id },
                last_write,
            })
        })
        .max_by_key(|found| found.last_write)
}

/// The MD5 of the directory, in hex.
fn cursor_agent_workspace_name(directory: &str) -> String {
    Md5::digest(directory.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The rollout last written to whose session ran in `directory`. Rollouts
/// are looked at newest first, so usually only a few are opened.
fn find_codex_chat(sessions: &Path, directory: &Path) -> Option<FoundChat> {
    let mut rollouts: Vec<(SystemTime, PathBuf)> = list_rollouts(sessions)
        .into_iter()
        .filter_map(|path| Some((last_write_of(&path)?, path)))
        .collect();
    rollouts.sort_by_key(|(last_write, _)| std::cmp::Reverse(*last_write));
    rollouts.into_iter().find_map(|(last_write, rollout)| {
        let session_id = session_id_if_ran_in(&read_first_line(&rollout)?, directory)?;
        Some(FoundChat {
            chat: LastChat::Codex { session_id },
            last_write,
        })
    })
}

/// `sessions/<year>/<month>/<day>/rollout-….jsonl`.
fn list_rollouts(sessions: &Path) -> Vec<PathBuf> {
    list_subdirectories_newest_first(sessions)
        .into_iter()
        .flat_map(|year| list_subdirectories_newest_first(&year))
        .flat_map(|month| list_subdirectories_newest_first(&month))
        .flat_map(|day| list_files(&day))
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(codex::is_rollout_name)
        })
        .collect()
}

/// The session's id, when the rollout's `session_meta` line names
/// `directory` as its `cwd`.
fn session_id_if_ran_in(session_meta_line: &str, directory: &Path) -> Option<String> {
    let session_meta: Value = serde_json::from_str(session_meta_line).ok()?;
    if session_meta["type"] != "session_meta" {
        return None;
    }
    let payload = &session_meta["payload"];
    if Path::new(payload["cwd"].as_str()?) != directory {
        return None;
    }
    let session_id = payload["id"].as_str()?;
    validate_chat_id(session_id).ok()?;
    Some(session_id.to_string())
}

fn read_first_line(path: &Path) -> Option<String> {
    let mut line = String::new();
    BufReader::new(File::open(path).ok()?)
        .read_line(&mut line)
        .ok()?;
    Some(line)
}

fn list_files(directory: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .collect()
}

fn last_write_of(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).ok()?.modified().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::Duration;

    fn write_file(path: &Path, contents: &str, seconds_after_epoch: u64) {
        fs::create_dir_all(path.parent().expect("the path has a parent"))
            .expect("creates directories");
        fs::write(path, contents).expect("writes the file");
        let modified = SystemTime::UNIX_EPOCH + Duration::from_secs(seconds_after_epoch);
        File::options()
            .write(true)
            .open(path)
            .and_then(|file| file.set_modified(modified))
            .expect("sets the modified time");
    }

    fn session_meta(id: &str, cwd: &str) -> String {
        format!(r#"{{"type":"session_meta","payload":{{"id":"{id}","cwd":"{cwd}"}}}}"#)
    }

    #[test]
    fn names_each_agents_directory_for_a_worktree() {
        assert_eq!(
            claude_code_project_name("/Users/me/code/code_reviewer/.worktrees/cr-x"),
            "-Users-me-code-code-reviewer--worktrees-cr-x"
        );
        assert_eq!(
            cursor_agent_workspace_name("/Users/chrislaskey/code/code_reviewer"),
            "0988cad20f19836c4a6c90ba2734a21f"
        );
    }

    #[test]
    fn types_the_resume_command_of_the_last_agent() {
        assert_eq!(continue_command(None), "claude --continue");
        assert_eq!(
            continue_command(Some(LastChat::ClaudeCode)),
            "claude --continue"
        );
        assert_eq!(
            continue_command(Some(LastChat::CursorAgent {
                chat_id: "74b6d800-ffac".into()
            })),
            "cursor-agent --resume 74b6d800-ffac"
        );
        assert_eq!(
            continue_command(Some(LastChat::Codex {
                session_id: "01a1e05c-d14d".into()
            })),
            "codex resume 01a1e05c-d14d"
        );
    }

    #[test]
    fn reads_the_session_id_only_of_rollouts_in_the_directory() {
        let directory = Path::new("/Users/me/code/app");
        assert_eq!(
            session_id_if_ran_in(
                &session_meta("01a1e05c-d14d", "/Users/me/code/app"),
                directory
            ),
            Some("01a1e05c-d14d".to_string())
        );
        assert_eq!(
            session_id_if_ran_in(&session_meta("01a1e05c-d14d", "/Users/me/other"), directory),
            None
        );
        assert_eq!(
            session_id_if_ran_in(&session_meta("../../etc", "/Users/me/code/app"), directory),
            None
        );
        assert_eq!(
            session_id_if_ran_in(r#"{"type":"response_item"}"#, directory),
            None
        );
    }

    #[test]
    fn finds_each_agents_newest_chat_in_the_directory() {
        let home = tempfile::tempdir().expect("creates a directory");
        let directory = Path::new("/Users/me/code/app");

        let projects = home.path().join("claude-projects");
        let project = projects.join(claude_code_project_name("/Users/me/code/app"));
        write_file(&project.join("old.jsonl"), "{}", 100);
        write_file(&project.join("new.jsonl"), "{}", 300);
        let claude_code = find_claude_code_chat(&projects, directory).expect("finds a chat");
        assert_eq!(
            claude_code.last_write,
            SystemTime::UNIX_EPOCH + Duration::from_secs(300)
        );

        let chats = home.path().join("cursor-chats");
        let workspace = chats.join(cursor_agent_workspace_name("/Users/me/code/app"));
        write_file(&workspace.join("chat-a/store.db"), "", 200);
        write_file(&workspace.join("chat-b/store.db"), "", 100);
        write_file(&workspace.join("chat-b/store.db-wal"), "", 400);
        let cursor_agent = find_cursor_agent_chat(&chats, directory).expect("finds a chat");
        assert_eq!(
            cursor_agent.chat,
            LastChat::CursorAgent {
                chat_id: "chat-b".into()
            }
        );

        let sessions = home.path().join("codex-sessions");
        let day = sessions.join("2026/11/15");
        write_file(
            &day.join("rollout-a.jsonl"),
            &session_meta("aaaa", "/Users/me/code/app"),
            100,
        );
        write_file(
            &day.join("rollout-b.jsonl"),
            &session_meta("bbbb", "/Users/me/code/app"),
            500,
        );
        write_file(
            &day.join("rollout-c.jsonl"),
            &session_meta("cccc", "/Users/me/other"),
            900,
        );
        let codex = find_codex_chat(&sessions, directory).expect("finds a chat");
        assert_eq!(
            codex.chat,
            LastChat::Codex {
                session_id: "bbbb".into()
            }
        );
        assert_eq!(
            codex.last_write,
            SystemTime::UNIX_EPOCH + Duration::from_secs(500)
        );

        assert!(find_claude_code_chat(&projects, Path::new("/Users/me/none")).is_none());
    }
}
