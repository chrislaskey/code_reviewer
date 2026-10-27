//! Claude Code sessions in tmux (`src/sessions/`): guesses which shell,
//! terminal app and tmux the user has, lists the tmux sessions the app
//! started with the Claude Code status in each, creates sessions (a git
//! worktree of a repository with a tmux session running a command in it)
//! and shows them in the terminal app.
//!
//! The app shares the user's tmux server, so it tells its own sessions apart
//! by name: every session it creates is named `<prefix>-<name>` (see
//! `SESSION_NAME_PREFIX`), and the list shows only those. Its git worktrees
//! are named the same, so after a restart, when the tmux sessions are gone,
//! they are found again in the repositories under the watched directories
//! and a session can be continued in them. A session is finished by merging
//! its branch or closing its worktree, as `bin/worktree` does.
//!
//! The settings are detected once per app run (see `AppState` in `lib.rs`)
//! and passed to every call.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use futures_util::future::join_all;
use serde::{Deserialize, Serialize};
use tokio::process::Command;

use crate::claude_code::{self, ClaudeCodeSession};
use crate::git::{self, GitStatus};
use crate::git_worktree::{self, Worktree};
use crate::process::{self, Process};
use crate::terminal_app::{self, InstalledTerminalApp, ShowOutcome, TerminalApp};
use crate::tmux::{self, Pane, Session, Tmux};

/// What the names of the sessions the app creates start with, as
/// `<prefix>-<name>`. The list shows only sessions and worktrees named so.
const SESSION_NAME_PREFIX: &str = "cr";
/// Where the app's worktrees are looked for: these directories and their
/// first-level subdirectories that are git repositories.
const WATCHED_DIRECTORIES: [&str; 1] = ["~/code"];
/// Typed into a session continued in an existing worktree.
const CONTINUE_COMMAND: &str = "claude --continue";
/// The branch a finished session's branch is merged into.
const MAIN_BRANCH: &str = "main";
const FALLBACK_SHELL: &str = "/bin/zsh";
/// Where tmux usually is: Homebrew on Apple Silicon, Homebrew on Intel and
/// MacPorts.
const COMMON_TMUX_PATHS: [&str; 3] = [
    "/opt/homebrew/bin/tmux",
    "/usr/local/bin/tmux",
    "/opt/local/bin/tmux",
];
/// How long the user's login shell may take to say where tmux is.
const LOGIN_SHELL_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Settings {
    pub shell: String,
    pub terminal_app: TerminalApp,
    pub tmux_path: String,
    /// Sessions and worktrees the app creates are named
    /// `<session_name_prefix>-<name>`.
    pub session_name_prefix: String,
    /// Where the app's worktrees are looked for, with `~` allowed.
    pub watched_directories: Vec<String>,
    /// The branch Merge fast-forwards with a session's branch.
    pub main_branch: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct DetectedSettings {
    pub settings: Settings,
    pub terminal_apps: Vec<InstalledTerminalApp>,
    /// The repository new sessions are worktrees of, unless the user picks
    /// another.
    pub directory: String,
    /// For showing paths in it as `~/...`.
    pub home_directory: Option<String>,
}

pub async fn detect_settings() -> DetectedSettings {
    let shell = detect_shell().await;
    let tmux_path = detect_tmux_path(&shell).await;
    let terminal_apps = terminal_app::list_terminal_apps().await;
    let terminal_app = terminal_apps
        .iter()
        .find(|app| app.path.is_some())
        .map_or(TerminalApp::Terminal, |app| app.id);

    DetectedSettings {
        settings: Settings {
            shell,
            terminal_app,
            tmux_path,
            session_name_prefix: SESSION_NAME_PREFIX.to_string(),
            watched_directories: WATCHED_DIRECTORIES.map(str::to_string).to_vec(),
            main_branch: MAIN_BRANCH.to_string(),
        },
        terminal_apps,
        directory: detect_directory().display().to_string(),
        home_directory: std::env::var("HOME").ok(),
    }
}

/// `name` as the app names its sessions: `<prefix>-<name>`.
pub fn add_session_name_prefix(prefix: &str, name: &str) -> String {
    format!("{prefix}-{name}")
}

/// Whether `session_name` is one the app created, named `<prefix>-<name>`.
pub fn has_session_name_prefix(prefix: &str, session_name: &str) -> bool {
    session_name
        .strip_prefix(prefix)
        .and_then(|rest| rest.strip_prefix('-'))
        .is_some_and(|name| !name.is_empty())
}

#[derive(Clone, Debug, Serialize)]
pub struct SessionList {
    pub sessions: Vec<SessionStatus>,
    /// The app's worktrees with no tmux session running in them, for a
    /// session to be continued in.
    pub worktrees: Vec<WorktreeStatus>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SessionStatus {
    #[serde(flatten)]
    pub session: Session,
    /// The directory of the session's current pane.
    pub directory: Option<String>,
    /// The Claude Code session running in one of the session's panes.
    pub claude_code: Option<ClaudeCodeSession>,
    /// The git status of the directory, when it's in a repository.
    pub git: Option<GitStatus>,
}

#[derive(Clone, Debug, Serialize)]
pub struct WorktreeStatus {
    #[serde(flatten)]
    pub worktree: Worktree,
    pub git: Option<GitStatus>,
}

/// The tmux sessions the app started (see `has_session_name_prefix`), each
/// with the Claude Code session running in it, and the app's worktrees
/// without a session.
pub async fn list_sessions(settings: &Settings) -> Result<SessionList, String> {
    let tmux = Tmux::new(&settings.tmux_path);
    let sessions: Vec<Session> = tmux
        .list_sessions()
        .await?
        .into_iter()
        .filter(|session| has_session_name_prefix(&settings.session_name_prefix, &session.name))
        .collect();

    let worktrees: Vec<Worktree> = list_worktrees(settings)
        .await
        .into_iter()
        .filter(|worktree| !sessions.iter().any(|session| session.name == worktree.name))
        .collect();
    let mut sessions = get_session_statuses(&tmux, sessions).await?;

    let directories = sessions
        .iter()
        .filter_map(|session| session.directory.as_deref())
        .chain(worktrees.iter().map(|worktree| worktree.path.as_str()));
    let git_statuses = get_git_statuses(directories).await;
    for session in &mut sessions {
        session.git = session
            .directory
            .as_ref()
            .and_then(|directory| git_statuses.get(directory).cloned());
    }
    let worktrees = worktrees
        .into_iter()
        .map(|worktree| WorktreeStatus {
            git: git_statuses.get(&worktree.path).cloned(),
            worktree,
        })
        .collect();

    Ok(SessionList {
        sessions,
        worktrees,
    })
}

/// The git status of each directory, all asked at the same time.
async fn get_git_statuses<'a>(
    directories: impl Iterator<Item = &'a str>,
) -> HashMap<String, GitStatus> {
    let mut directories: Vec<&str> = directories.collect();
    directories.sort_unstable();
    directories.dedup();

    let statuses = join_all(
        directories
            .iter()
            .map(|directory| git::get_status(directory)),
    )
    .await;
    directories
        .into_iter()
        .zip(statuses)
        .filter_map(|(directory, status)| Some((directory.to_string(), status?)))
        .collect()
}

/// The app's worktrees (named with the prefix) in the repositories under the
/// watched directories.
async fn list_worktrees(settings: &Settings) -> Vec<Worktree> {
    let directories: Vec<PathBuf> = settings
        .watched_directories
        .iter()
        .map(|directory| expand_home(directory))
        .collect();
    git_worktree::list_worktrees(&directories)
        .await
        .into_iter()
        .filter(|worktree| has_session_name_prefix(&settings.session_name_prefix, &worktree.name))
        .collect()
}

/// Each session with the Claude Code session running in it. Panes and
/// processes are listed only when there are sessions to look at.
async fn get_session_statuses(
    tmux: &Tmux,
    sessions: Vec<Session>,
) -> Result<Vec<SessionStatus>, String> {
    if sessions.is_empty() {
        return Ok(Vec::new());
    }
    let panes = tmux.list_panes().await?;
    let processes = process::list_processes().await?;
    let claude_code_sessions = claude_code::list_sessions(&processes).await;

    Ok(sessions
        .into_iter()
        .map(|session| {
            let session_panes: Vec<&Pane> = panes
                .iter()
                .filter(|pane| pane.session_name == session.name)
                .collect();
            SessionStatus {
                directory: current_pane(&session_panes).map(|pane| pane.current_path.clone()),
                claude_code: find_claude_code(&session_panes, &processes, &claude_code_sessions),
                session,
                git: None,
            }
        })
        .collect())
}

/// The current pane of the session's current window.
fn current_pane<'a>(session_panes: &[&'a Pane]) -> Option<&'a Pane> {
    session_panes
        .iter()
        .find(|pane| pane.window_active && pane.active)
        .or(session_panes.first())
        .copied()
}

/// The Claude Code session running in one of the session's panes: the one
/// whose status file names the pane, else a `claude` process on a pane's
/// tty, which has no status yet (its file isn't written, or it's a Claude
/// Code that writes none). `claude_code_sessions` come most recently updated
/// first, so that one wins when several run in the session.
fn find_claude_code(
    session_panes: &[&Pane],
    processes: &[Process],
    claude_code_sessions: &[ClaudeCodeSession],
) -> Option<ClaudeCodeSession> {
    let reported = claude_code_sessions
        .iter()
        .find(|session| session_panes.iter().any(|pane| runs_in_pane(session, pane)));
    if let Some(session) = reported {
        return Some(session.clone());
    }

    processes
        .iter()
        .filter(|process| process.is_claude_code())
        .find(|process| {
            session_panes
                .iter()
                .any(|pane| process.tty.as_deref() == Some(pane.tty.as_str()))
        })
        .map(|process| ClaudeCodeSession::without_status_file(process.pid))
}

fn runs_in_pane(session: &ClaudeCodeSession, pane: &Pane) -> bool {
    session.tmux_pane_id.as_deref() == Some(pane.id.as_str())
        && session.tmux_session_name.as_deref() == Some(pane.session_name.as_str())
}

/// Creates a session named `<prefix>-<name>`: a git worktree of
/// `repository` on a branch of that same name (see
/// `git_worktree::add_worktree`) and a tmux session running the user's
/// shell in it, with `command` typed in, opened in the terminal app.
pub async fn create_session(
    settings: &Settings,
    name: &str,
    repository: &str,
    command: &str,
) -> Result<ShowOutcome, String> {
    let name = name.trim();
    tmux::validate_session_name(name)?;
    let session_name = add_session_name_prefix(&settings.session_name_prefix, name);
    tmux::validate_session_name(&session_name)?;
    let repository = expand_home(repository.trim());
    if !repository.is_dir() {
        return Err(format!("{} is not a directory", repository.display()));
    }
    let repository_root = git_worktree::find_repository_root(&repository).await?;

    let tmux = Tmux::new(&settings.tmux_path);
    if tmux.has_session(&session_name).await? {
        return Err(format!(
            "A session named {session_name} already exists, open it from the list"
        ));
    }

    if git_worktree::worktree_path(&repository_root, &session_name).exists() {
        return Err(format!(
            "A worktree named {session_name} already exists in {}, continue it from the list",
            repository_root.display()
        ));
    }

    let worktree = git_worktree::add_worktree(&repository_root, &session_name).await?;
    log::info!(
        "Added the worktree {} on the {} branch {}",
        worktree.path.display(),
        if worktree.branch_created {
            "new"
        } else {
            "existing"
        },
        worktree.branch
    );
    start_session_in_directory(settings, &session_name, &worktree.path, command).await
}

/// Starts a session again in one of the app's worktrees (see
/// `SessionList::worktrees`): a tmux session named after the worktree,
/// running the user's shell in it with `CONTINUE_COMMAND` typed in, opened
/// in the terminal app. `name` is the worktree's name, prefix included.
pub async fn continue_session(settings: &Settings, name: &str) -> Result<ShowOutcome, String> {
    let worktree = find_worktree(settings, name).await?;
    let tmux = Tmux::new(&settings.tmux_path);
    if tmux.has_session(name).await? {
        return Err(format!(
            "A session named {name} is already running, open it from the list"
        ));
    }

    start_session_in_directory(settings, name, Path::new(&worktree.path), CONTINUE_COMMAND).await
}

/// Finishes a session by merging its branch into the main branch, a
/// fast-forward only (see `git_worktree::merge_worktree`), removing its
/// worktree, deleting the branch and closing its tmux session, like
/// `bin/worktree merge`. `name` is the worktree's name, prefix included.
/// Returns what was done, for the log.
pub async fn merge_session(settings: &Settings, name: &str) -> Result<String, String> {
    let worktree = find_worktree(settings, name).await?;
    git_worktree::merge_worktree(&worktree, &settings.main_branch).await?;
    let session_closed = close_tmux_session_if_running(settings, name).await?;
    Ok(format!(
        "Merged {name} into {}, removed the worktree and deleted the branch{session_closed}",
        settings.main_branch
    ))
}

/// Finishes a session by removing its worktree, keeping the branch, and
/// closing its tmux session, like `bin/worktree close`. `name` is the
/// worktree's name, prefix included. Returns what was done, for the log.
pub async fn close_session(settings: &Settings, name: &str) -> Result<String, String> {
    let worktree = find_worktree(settings, name).await?;
    git_worktree::close_worktree(&worktree, &settings.main_branch).await?;
    let session_closed = close_tmux_session_if_running(settings, name).await?;
    Ok(format!(
        "Removed the worktree of {name}, keeping its branch{session_closed}"
    ))
}

/// One of the app's worktrees, by name.
async fn find_worktree(settings: &Settings, name: &str) -> Result<Worktree, String> {
    tmux::validate_session_name(name)?;
    if !has_session_name_prefix(&settings.session_name_prefix, name) {
        return Err(format!("{name} is not a worktree this app created"));
    }
    list_worktrees(settings)
        .await
        .into_iter()
        .find(|worktree| worktree.name == name)
        .ok_or_else(|| format!("There is no worktree named {name} under the watched directories"))
}

/// Closes the tmux session `name` when it's running. Returns a phrase for
/// the log.
async fn close_tmux_session_if_running(
    settings: &Settings,
    name: &str,
) -> Result<&'static str, String> {
    let tmux = Tmux::new(&settings.tmux_path);
    if !tmux.has_session(name).await? {
        return Ok("");
    }
    tmux.kill_session(name).await?;
    Ok(", and closed the tmux session")
}

/// Creates the tmux session `session_name` running the user's shell in
/// `directory`, types `command` into it and opens it in the terminal app.
async fn start_session_in_directory(
    settings: &Settings,
    session_name: &str,
    directory: &Path,
    command: &str,
) -> Result<ShowOutcome, String> {
    let tmux = Tmux::new(&settings.tmux_path);
    tmux.create_session(
        session_name,
        &settings.shell,
        &directory.display().to_string(),
    )
    .await?;
    tmux.show_session_in_window_title(session_name).await?;
    if !command.trim().is_empty() {
        tmux.send_command(session_name, command.trim()).await?;
    }

    terminal_app::open_window(settings.terminal_app, &settings.tmux_path, session_name).await?;
    Ok(ShowOutcome::Opened)
}

/// Brings the terminal window of one of the app's sessions to the front, or
/// opens one. `name` is the full session name, prefix included. The window
/// title is set again in case the session was created by an earlier run.
pub async fn show_session_in_terminal_app(
    settings: &Settings,
    name: &str,
) -> Result<ShowOutcome, String> {
    tmux::validate_session_name(name)?;
    if !has_session_name_prefix(&settings.session_name_prefix, name) {
        return Err(format!("{name} is not a session this app started"));
    }
    let tmux = Tmux::new(&settings.tmux_path);
    if !tmux.has_session(name).await? {
        return Err(format!("There is no session named {name}"));
    }

    tmux.show_session_in_window_title(name).await?;
    let client_ttys = tmux.list_client_ttys(name).await?;
    terminal_app::show_tmux_session(
        settings.terminal_app,
        &settings.tmux_path,
        name,
        &client_ttys,
    )
    .await
}

/// The root of the git repository the app runs in, else the directory it
/// runs in. Apps launched from Finder run in `/`, so then the user's home
/// directory.
fn detect_directory() -> PathBuf {
    let home = std::env::var("HOME").map(PathBuf::from).ok();
    std::env::current_dir()
        .ok()
        .filter(|directory| directory != Path::new("/"))
        .map(|directory| find_git_root(&directory).unwrap_or(directory))
        .or(home)
        .unwrap_or_else(|| PathBuf::from("/"))
}

/// The nearest directory at or above `directory` with a `.git` entry. It's a
/// file in worktrees and submodules, a directory otherwise.
fn find_git_root(directory: &Path) -> Option<PathBuf> {
    directory
        .ancestors()
        .find(|ancestor| ancestor.join(".git").exists())
        .map(Path::to_path_buf)
}

/// Expands a leading `~` to the user's home directory, like a shell does.
fn expand_home(path: &str) -> PathBuf {
    let Ok(home) = std::env::var("HOME") else {
        return PathBuf::from(path);
    };
    match path.strip_prefix('~') {
        Some("") => PathBuf::from(home),
        Some(rest) if rest.starts_with('/') => Path::new(&home).join(&rest[1..]),
        _ => PathBuf::from(path),
    }
}

/// The user's login shell. Apps launched from Finder get `SHELL` too.
async fn detect_shell() -> String {
    if let Ok(shell) = std::env::var("SHELL")
        && !shell.is_empty()
    {
        return shell;
    }

    let user = std::env::var("USER").unwrap_or_default();
    let output = Command::new("dscl")
        .args([".", "-read", &format!("/Users/{user}"), "UserShell"])
        .output()
        .await;
    output
        .ok()
        .and_then(|output| {
            String::from_utf8_lossy(&output.stdout)
                .strip_prefix("UserShell:")
                .map(|shell| shell.trim().to_string())
        })
        .filter(|shell| !shell.is_empty())
        .unwrap_or_else(|| FALLBACK_SHELL.to_string())
}

/// Asks the user's login shell, since an app launched from Finder doesn't
/// get the PATH the user's terminal has, then tries the usual places.
async fn detect_tmux_path(shell: &str) -> String {
    let login_shell = Command::new(shell)
        .args(["-l", "-c", "command -v tmux"])
        .kill_on_drop(true)
        .output();
    if let Ok(Ok(output)) = tokio::time::timeout(LOGIN_SHELL_TIMEOUT, login_shell).await {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if let Some(path) = stdout
            .lines()
            .map(str::trim)
            .find(|line| line.starts_with('/'))
        {
            return path.to_string();
        }
    }

    COMMON_TMUX_PATHS
        .into_iter()
        .find(|path| std::path::Path::new(path).exists())
        .unwrap_or("tmux")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pane(session_name: &str, id: &str, tty: &str, active: bool) -> Pane {
        Pane {
            session_name: session_name.into(),
            window_index: 0,
            window_name: "zsh".into(),
            window_active: true,
            id: id.into(),
            index: 0,
            active,
            pid: 1,
            tty: tty.into(),
            dead: false,
            current_command: "zsh".into(),
            current_path: format!("/code/{id}"),
        }
    }

    fn claude_process(pid: u32, tty: &str) -> Process {
        Process {
            pid,
            parent_pid: 1,
            process_group_id: pid,
            foreground_process_group_id: pid,
            in_foreground: true,
            tty: Some(tty.into()),
            state_code: "S+".into(),
            state: crate::process::ProcessState::Sleeping,
            cpu_percent: 0.0,
            name: "claude".into(),
            command: "/Users/me/.local/share/claude/versions/2.1.309".into(),
        }
    }

    fn reported_session(pid: u32, tmux_location: &str) -> ClaudeCodeSession {
        ClaudeCodeSession {
            status: Some("busy".into()),
            tmux_session_name: Some(tmux_location.split(':').next().unwrap_or("").into()),
            tmux_pane_id: tmux_location.rsplit('.').next().map(str::to_string),
            tmux_location: Some(tmux_location.into()),
            ..ClaudeCodeSession::without_status_file(pid)
        }
    }

    #[test]
    fn names_sessions_with_the_prefix() {
        assert_eq!(add_session_name_prefix("cr", "claude"), "cr-claude");
        assert!(has_session_name_prefix("cr", "cr-claude"));
        assert!(!has_session_name_prefix("cr", "cr-"));
        assert!(!has_session_name_prefix("cr", "cr"));
        assert!(!has_session_name_prefix("cr", "crm-claude"));
        assert!(!has_session_name_prefix("cr", "claude"));
    }

    #[test]
    fn finds_the_claude_code_session_in_a_sessions_panes() {
        let panes = [
            pane("cr-review", "%1", "ttys001", false),
            pane("cr-review", "%2", "ttys002", true),
            pane("other", "%3", "ttys003", true),
        ];
        let review_panes: Vec<&Pane> = panes.iter().take(2).collect();
        let other_panes: Vec<&Pane> = panes.iter().skip(2).collect();
        let processes = [claude_process(10, "ttys002"), claude_process(30, "ttys003")];
        let reported = [reported_session(30, "other:@1.%3")];

        let in_review = find_claude_code(&review_panes, &processes, &reported)
            .expect("the claude process on the pane's tty is found");
        assert_eq!(in_review.pid, 10);
        assert_eq!(in_review.status, None);

        let in_other = find_claude_code(&other_panes, &processes, &reported)
            .expect("the reported session is found");
        assert_eq!(in_other.pid, 30);
        assert_eq!(in_other.status.as_deref(), Some("busy"));

        assert_eq!(find_claude_code(&[], &processes, &reported), None);
        assert_eq!(
            current_pane(&review_panes).map(|pane| pane.id.as_str()),
            Some("%2")
        );
    }

    #[test]
    fn expands_a_leading_tilde() {
        let home = std::env::var("HOME").expect("HOME is set");
        assert_eq!(expand_home("~"), PathBuf::from(&home));
        assert_eq!(expand_home("~/code"), Path::new(&home).join("code"));
        assert_eq!(expand_home("/tmp/~"), PathBuf::from("/tmp/~"));
        assert_eq!(expand_home("~other/code"), PathBuf::from("~other/code"));
    }

    #[test]
    fn finds_the_nearest_git_root() {
        let root =
            std::env::temp_dir().join(format!("code-reviewer-git-root-{}", std::process::id()));
        let nested = root.join("repo/sub/dir");
        std::fs::create_dir_all(&nested).expect("directories are created");
        std::fs::create_dir(root.join("repo/.git")).expect(".git is created");

        let found = find_git_root(&nested);
        let outside = find_git_root(&root);
        std::fs::remove_dir_all(&root).expect("directories are removed");

        assert_eq!(found, Some(root.join("repo")));
        assert_eq!(outside, None);
    }
}
