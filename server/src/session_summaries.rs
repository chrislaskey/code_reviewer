//! The sessions page's list: each of the app's tmux sessions with its
//! agent's status, and the worktrees with no session, without the git
//! status that takes longest to find. Both parts are kept in memory and
//! served at once: one older than its `max_age` is served still, and
//! refreshed in the background for the next call, so the page shows what
//! was found up to one refresh ago. Every viewer shares the one refresh.
//! See decision 27 in `docs/decisions.md`.

use std::future::Future;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::agent::AgentSession;
use crate::git_worktree::Worktree;
use crate::recent_writes::OpenFileCache;
use crate::sessions::{self, SessionAgent, Settings};

/// Finding the agents takes a tmux and a `ps` call, so it's done often.
const SESSION_AGENTS_MAX_AGE: Duration = Duration::from_secs(1);
/// Finding the worktrees runs git in every repository watched.
const WORKTREES_MAX_AGE: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct SessionSummaries {
    pub sessions: Vec<SessionSummary>,
    /// The app's worktrees with no tmux session running in them, for a
    /// session to be continued in.
    pub worktrees: Vec<Worktree>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct SessionSummary {
    /// The full session name, prefix included.
    pub name: String,
    /// The repository the session's worktree is of, else the name of the
    /// directory it runs in.
    pub repository_name: Option<String>,
    pub agent: Option<AgentSession>,
    /// Whether the asking device's bell is on for the session.
    #[serde(default)]
    pub notifications_enabled: bool,
}

/// The two parts of the list, each refreshed on its own.
pub struct SessionSummaryCache {
    open_files: Arc<OpenFileCache>,
    session_agents: Arc<Cached<Vec<SessionAgent>>>,
    worktrees: Arc<Cached<Vec<Worktree>>>,
}

impl SessionSummaryCache {
    /// An empty cache: the first call waits for both parts. `open_files`
    /// is shared with the other lookups of agents.
    pub fn new(open_files: Arc<OpenFileCache>) -> Self {
        Self {
            open_files,
            session_agents: Arc::new(Cached::new(SESSION_AGENTS_MAX_AGE)),
            worktrees: Arc::new(Cached::new(WORKTREES_MAX_AGE)),
        }
    }

    /// The list as last found, refreshing the parts that are old.
    pub async fn list_session_summaries(
        &self,
        settings: &Settings,
    ) -> Result<SessionSummaries, String> {
        let agents_settings = settings.clone();
        let worktrees_settings = settings.clone();
        let open_files = Arc::clone(&self.open_files);
        let (session_agents, worktrees) = tokio::join!(
            self.session_agents.get(|| async move {
                sessions::list_session_agents(&agents_settings, &open_files).await
            }),
            self.worktrees
                .get(|| async move { Ok(sessions::list_worktrees(&worktrees_settings).await) }),
        );
        Ok(summarize(session_agents?, worktrees?))
    }

    /// Drops both parts, so the next call waits for them to be found again:
    /// for after a session is created, continued or finished.
    pub async fn forget(&self) {
        tokio::join!(self.session_agents.forget(), self.worktrees.forget());
    }
}

fn summarize(session_agents: Vec<SessionAgent>, worktrees: Vec<Worktree>) -> SessionSummaries {
    let sessions = session_agents
        .iter()
        .map(|session_agent| SessionSummary {
            name: session_agent.session.name.clone(),
            repository_name: repository_name_of(session_agent, &worktrees),
            agent: session_agent.agent.clone(),
            notifications_enabled: false,
        })
        .collect();
    let worktrees = worktrees
        .into_iter()
        .filter(|worktree| {
            !session_agents
                .iter()
                .any(|session_agent| session_agent.session.name == worktree.name)
        })
        .collect();
    SessionSummaries {
        sessions,
        worktrees,
    }
}

/// The app's worktrees are named after their sessions.
fn repository_name_of(session_agent: &SessionAgent, worktrees: &[Worktree]) -> Option<String> {
    let path = worktrees
        .iter()
        .find(|worktree| worktree.name == session_agent.session.name)
        .map(|worktree| worktree.repository.as_str())
        .or(session_agent.directory.as_deref())?;
    let name = Path::new(path).file_name()?.to_str()?;
    Some(name.to_string())
}

/// A value found by a slow call, kept until it's `max_age` old, and served
/// after that while a refresh runs in the background.
struct Cached<T> {
    max_age: Duration,
    state: Mutex<CachedState<T>>,
}

struct CachedState<T> {
    value: Option<(Instant, T)>,
    refreshing: bool,
    /// Counts the `forget` calls, so a refresh started before one doesn't
    /// keep what it found.
    generation: u64,
}

enum NextStep<T> {
    Serve(T),
    ServeAndRefresh(T, u64),
    WaitForRefresh(u64),
}

impl<T: Clone + Send + 'static> Cached<T> {
    fn new(max_age: Duration) -> Self {
        Self {
            max_age,
            state: Mutex::new(CachedState {
                value: None,
                refreshing: false,
                generation: 0,
            }),
        }
    }

    /// The value kept, else waits for `find`. A value older than `max_age`
    /// is served and `find` runs in the background.
    async fn get<F, Found>(self: &Arc<Self>, find: F) -> Result<T, String>
    where
        F: FnOnce() -> Found,
        Found: Future<Output = Result<T, String>> + Send + 'static,
    {
        let next_step = self.choose_next_step().await;
        match next_step {
            NextStep::Serve(value) => Ok(value),
            NextStep::ServeAndRefresh(value, generation) => {
                let cached = Arc::clone(self);
                let found = find();
                tokio::spawn(async move {
                    let result = found.await;
                    cached.finish_background_refresh(generation, result).await;
                });
                Ok(value)
            }
            NextStep::WaitForRefresh(generation) => {
                let value = find().await?;
                self.keep(generation, value.clone()).await;
                Ok(value)
            }
        }
    }

    async fn choose_next_step(&self) -> NextStep<T> {
        let mut state = self.state.lock().await;
        let generation = state.generation;
        match &state.value {
            None => NextStep::WaitForRefresh(generation),
            Some((found_at, value)) if found_at.elapsed() <= self.max_age || state.refreshing => {
                NextStep::Serve(value.clone())
            }
            Some((_, value)) => {
                let value = value.clone();
                state.refreshing = true;
                NextStep::ServeAndRefresh(value, generation)
            }
        }
    }

    async fn finish_background_refresh(&self, generation: u64, result: Result<T, String>) {
        match result {
            Ok(value) => self.keep(generation, value).await,
            Err(error) => log::warn!("Refreshing the sessions list failed: {error}"),
        }
        self.state.lock().await.refreshing = false;
    }

    async fn keep(&self, generation: u64, value: T) {
        let mut state = self.state.lock().await;
        if state.generation == generation {
            state.value = Some((Instant::now(), value));
        }
    }

    async fn forget(&self) {
        let mut state = self.state.lock().await;
        state.value = None;
        state.generation += 1;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::*;
    use crate::agent::AgentKind;
    use crate::tmux::Session;

    fn session_agent(name: &str, directory: &str, status: &str) -> SessionAgent {
        SessionAgent {
            session: Session {
                name: name.into(),
                window_count: 1,
                attached_clients: 0,
                current_command: "zsh".into(),
            },
            directory: Some(directory.into()),
            agent: Some(AgentSession {
                kind: AgentKind::ClaudeCode,
                pid: 1,
                session_id: None,
                tmux_pane_id: None,
                status: Some(status.into()),
                status_updated_at: None,
            }),
        }
    }

    fn worktree(name: &str, repository: &str) -> Worktree {
        Worktree {
            name: name.into(),
            path: format!("{repository}/.worktrees/{name}"),
            branch: Some(name.into()),
            repository: repository.into(),
        }
    }

    #[test]
    fn lists_worktrees_with_no_session_and_names_each_sessions_repository() {
        let summaries = summarize(
            vec![
                session_agent("cr-a", "/code/app/.worktrees/cr-a", "busy"),
                session_agent("cr-b", "/code/notes", "idle"),
            ],
            vec![worktree("cr-a", "/code/app"), worktree("cr-c", "/code/app")],
        );

        let names: Vec<_> = summaries
            .sessions
            .iter()
            .map(|session| (session.name.as_str(), session.repository_name.as_deref()))
            .collect();
        assert_eq!(names, [("cr-a", Some("app")), ("cr-b", Some("notes"))]);
        assert_eq!(summaries.worktrees, [worktree("cr-c", "/code/app")]);
    }

    #[tokio::test]
    async fn serves_an_old_value_while_it_refreshes_and_waits_after_forget() {
        let cached = Arc::new(Cached::new(Duration::ZERO));
        let calls = Arc::new(AtomicU32::new(0));
        let find = || {
            let calls = Arc::clone(&calls);
            async move { Ok(calls.fetch_add(1, Ordering::SeqCst) + 1) }
        };

        assert_eq!(cached.get(find).await, Ok(1));
        assert_eq!(cached.get(find).await, Ok(1));
        while cached.state.lock().await.refreshing {
            tokio::task::yield_now().await;
        }
        assert_eq!(cached.get(find).await, Ok(2));

        cached.forget().await;
        let after_forget = cached.get(find).await;
        assert!(matches!(after_forget, Ok(calls) if calls >= 3));
    }
}
