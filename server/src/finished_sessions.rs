//! Watches the sessions with a bell on and queues a notification each time
//! one's agent finishes: goes from busy to not busy. Claude Code reports its
//! own status; Cursor's and Codex's are guessed from their writes, so they
//! must stay quiet a while first, or a long tool call would count.
//! See decision 29 in `docs/decisions.md`.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::agent::AgentKind;
use crate::agent_chat::{ChatMessage, ChatPart, ChatRole, shorten};
use crate::backend::Backend;
use crate::session_notifications::SessionNotification;
use crate::session_summaries::SessionSummary;

const CHECK_INTERVAL: Duration = Duration::from_secs(2);
/// How long a guessed status must stay not busy to count as finished.
const GUESSED_STATUS_SETTLE_TIME: Duration = Duration::from_secs(30);
/// Checks in a row a session must be missing from the list before its
/// bells are turned off, as the list can be one refresh old.
const CHECKS_MISSING_BEFORE_GONE: u32 = 2;
const MAX_BODY_CHARS: usize = 120;
const BODY_WHEN_NO_MESSAGE: &str = "Finished";

/// Checks the watched sessions every few seconds, for as long as the server runs.
pub async fn watch_for_finished_sessions(backend: Arc<Backend>) {
    let mut detector = FinishDetector::default();
    let mut interval = tokio::time::interval(CHECK_INTERVAL);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        interval.tick().await;
        check_watched_sessions(&backend, &mut detector).await;
    }
}

async fn check_watched_sessions(backend: &Backend, detector: &mut FinishDetector) {
    let notifications = backend.session_notifications();
    let watched = notifications.list_watched_session_names();
    if watched.is_empty() {
        detector.sessions.clear();
        return;
    }
    let summaries = match backend.list_cached_session_summaries().await {
        Ok(summaries) => summaries,
        Err(error) => {
            log::warn!("Checking the watched sessions failed: {error}");
            return;
        }
    };
    let observation = detector.observe(&watched, &summaries.sessions, Instant::now());
    if !observation.gone.is_empty() {
        log::info!(
            "Turning off the bells of ended sessions: {}",
            observation.gone.join(", ")
        );
    }
    if !observation.gone.is_empty()
        && let Err(error) = notifications.remove_sessions(&observation.gone)
    {
        log::warn!("Turning off the bells of ended sessions failed: {error}");
    }
    for name in observation.finished {
        log::info!("The agent in {name} finished; queueing its notification");
        let body = backend
            .get_agent_chat_history(&name)
            .await
            .ok()
            .and_then(|chat| last_message_line(&chat.messages))
            .unwrap_or_else(|| BODY_WHEN_NO_MESSAGE.to_string());
        let notification = SessionNotification {
            title: name.clone(),
            session_name: name,
            body,
        };
        notifications.add(notification.clone());
        backend.send_apple_push_notifications(&notification).await;
    }
}

/// What the watched sessions were doing at the last check.
#[derive(Default)]
struct FinishDetector {
    sessions: HashMap<String, WatchedSession>,
}

#[derive(Default)]
struct WatchedSession {
    /// Busy since it was last found finished, or since watching began.
    busy_seen: bool,
    not_busy_since: Option<Instant>,
    checks_missing: u32,
}

#[derive(Debug, Default, PartialEq)]
struct Observation {
    /// Sessions whose agent just finished.
    finished: Vec<String>,
    /// Sessions no longer running.
    gone: Vec<String>,
}

impl FinishDetector {
    fn observe(
        &mut self,
        watched: &[String],
        listed: &[SessionSummary],
        now: Instant,
    ) -> Observation {
        self.sessions.retain(|name, _| watched.contains(name));
        let mut observation = Observation::default();
        for name in watched {
            let session = self.sessions.entry(name.clone()).or_default();
            let Some(summary) = listed.iter().find(|summary| &summary.name == name) else {
                session.checks_missing += 1;
                if session.checks_missing >= CHECKS_MISSING_BEFORE_GONE {
                    observation.gone.push(name.clone());
                }
                continue;
            };
            session.checks_missing = 0;
            if session.notice(summary, now) {
                observation.finished.push(name.clone());
            }
        }
        for name in &observation.gone {
            self.sessions.remove(name);
        }
        observation
    }
}

impl WatchedSession {
    /// Whether the agent finished since the last check.
    fn notice(&mut self, summary: &SessionSummary, now: Instant) -> bool {
        let agent = summary.agent.as_ref();
        if agent.is_some_and(|agent| agent.status.as_deref() == Some("busy")) {
            self.busy_seen = true;
            self.not_busy_since = None;
            return false;
        }
        if !self.busy_seen {
            return false;
        }
        let not_busy_since = *self.not_busy_since.get_or_insert(now);
        let settle_time = match agent.map(|agent| agent.kind) {
            Some(AgentKind::CursorAgent | AgentKind::Codex) => GUESSED_STATUS_SETTLE_TIME,
            Some(AgentKind::ClaudeCode) | None => Duration::ZERO,
        };
        if now.duration_since(not_busy_since) < settle_time {
            return false;
        }
        self.busy_seen = false;
        self.not_busy_since = None;
        true
    }
}

/// The first line of the newest assistant message with text, as plain text.
fn last_message_line(messages: &[ChatMessage]) -> Option<String> {
    messages
        .iter()
        .rev()
        .filter(|message| message.role == ChatRole::Assistant)
        .flat_map(|message| &message.parts)
        .find_map(|part| match part {
            ChatPart::Text { html } => first_line_of_html(html),
            _ => None,
        })
}

fn first_line_of_html(html: &str) -> Option<String> {
    let mut text = String::new();
    let mut rest = html;
    while let Some(start) = rest.find('<') {
        text.push_str(&rest[..start]);
        let Some(length) = rest[start..].find('>') else {
            break;
        };
        let tag = &rest[start + 1..start + length];
        if ["/p", "/h", "/li", "/pre", "/blockquote", "br", "/div"]
            .iter()
            .any(|block_end| tag.starts_with(block_end))
        {
            text.push('\n');
        }
        rest = &rest[start + length + 1..];
    }
    text.push_str(rest);
    let line = text.lines().map(str::trim).find(|line| !line.is_empty())?;
    Some(shorten(&decode_html_entities(line), MAX_BODY_CHARS))
}

fn decode_html_entities(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::AgentSession;

    fn summary(name: &str, kind: AgentKind, status: &str) -> SessionSummary {
        SessionSummary {
            name: name.into(),
            repository_name: None,
            agent: Some(AgentSession {
                kind,
                pid: 1,
                session_id: None,
                tmux_pane_id: None,
                status: Some(status.into()),
                status_updated_at: None,
            }),
            notifications_enabled: false,
        }
    }

    fn watched() -> Vec<String> {
        vec!["cr-a".to_string()]
    }

    #[test]
    fn claude_code_finishes_at_once_and_again_after_each_busy_spell() {
        let mut detector = FinishDetector::default();
        let start = Instant::now();
        let busy = [summary("cr-a", AgentKind::ClaudeCode, "busy")];
        let idle = [summary("cr-a", AgentKind::ClaudeCode, "idle")];

        assert!(
            detector
                .observe(&watched(), &idle, start)
                .finished
                .is_empty()
        );
        assert!(
            detector
                .observe(&watched(), &busy, start)
                .finished
                .is_empty()
        );
        assert_eq!(
            detector.observe(&watched(), &idle, start).finished,
            ["cr-a"]
        );
        assert!(
            detector
                .observe(&watched(), &idle, start)
                .finished
                .is_empty()
        );
        detector.observe(&watched(), &busy, start);
        assert_eq!(
            detector.observe(&watched(), &idle, start).finished,
            ["cr-a"]
        );
    }

    #[test]
    fn a_guessed_status_finishes_only_after_staying_quiet() {
        let mut detector = FinishDetector::default();
        let start = Instant::now();
        let busy = [summary("cr-a", AgentKind::Codex, "busy")];
        let idle = [summary("cr-a", AgentKind::Codex, "idle")];

        detector.observe(&watched(), &busy, start);
        assert!(
            detector
                .observe(&watched(), &idle, start)
                .finished
                .is_empty()
        );
        let quiet_tool_call_over = start + Duration::from_secs(10);
        detector.observe(&watched(), &busy, quiet_tool_call_over);
        detector.observe(&watched(), &idle, quiet_tool_call_over);
        let almost = quiet_tool_call_over + GUESSED_STATUS_SETTLE_TIME - Duration::from_secs(1);
        assert!(
            detector
                .observe(&watched(), &idle, almost)
                .finished
                .is_empty()
        );
        let settled = quiet_tool_call_over + GUESSED_STATUS_SETTLE_TIME;
        assert_eq!(
            detector.observe(&watched(), &idle, settled).finished,
            ["cr-a"]
        );
    }

    #[test]
    fn an_agent_that_exits_while_busy_has_finished() {
        let mut detector = FinishDetector::default();
        let now = Instant::now();
        let exited = [SessionSummary {
            agent: None,
            ..summary("cr-a", AgentKind::Codex, "idle")
        }];

        detector.observe(
            &watched(),
            &[summary("cr-a", AgentKind::Codex, "busy")],
            now,
        );

        assert_eq!(
            detector.observe(&watched(), &exited, now).finished,
            ["cr-a"]
        );
    }

    #[test]
    fn a_session_missing_from_two_checks_in_a_row_is_gone() {
        let mut detector = FinishDetector::default();
        let now = Instant::now();
        let listed = [summary("cr-a", AgentKind::ClaudeCode, "idle")];

        assert!(detector.observe(&watched(), &[], now).gone.is_empty());
        assert!(detector.observe(&watched(), &listed, now).gone.is_empty());
        assert!(detector.observe(&watched(), &[], now).gone.is_empty());
        assert_eq!(detector.observe(&watched(), &[], now).gone, ["cr-a"]);
    }

    fn message(role: ChatRole, html: &str) -> ChatMessage {
        ChatMessage {
            id: "1".into(),
            role,
            timestamp: None,
            note: None,
            parts: vec![ChatPart::Text { html: html.into() }],
        }
    }

    #[test]
    fn the_body_is_the_first_line_of_the_newest_assistant_text() {
        let messages = [
            message(ChatRole::Assistant, "<p>Older</p>"),
            message(
                ChatRole::Assistant,
                "<h2>Done: <code>a &amp; b</code></h2>\n<p>More</p>",
            ),
            message(ChatRole::User, "<p>Thanks</p>"),
        ];

        assert_eq!(last_message_line(&messages).as_deref(), Some("Done: a & b"));
    }

    #[test]
    fn a_long_line_is_shortened() {
        let html = format!("<p>{}</p>", "x".repeat(200));

        let line = first_line_of_html(&html).unwrap();

        assert_eq!(line.chars().count(), MAX_BODY_CHARS + 1);
        assert!(line.ends_with('…'));
    }
}
