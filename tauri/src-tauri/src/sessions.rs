//! Claude Code sessions in tmux (`src/sessions/`): guesses which shell,
//! terminal app and tmux the user has, lists their tmux sessions with what
//! runs in each pane and their Claude Code sessions, creates sessions running
//! a command and shows them in the terminal app.
//!
//! The settings are detected once per app run (see `AppState` in `lib.rs`)
//! and passed to every call.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::process::Command;

use futures_util::future::join_all;

use crate::claude_code::{self, ClaudeCodeSession};
use crate::git::{self, GitStatus};
use crate::process::{self, Process};
use crate::terminal_app::{self, InstalledTerminalApp, ShowOutcome, TerminalApp};
use crate::tmux::{self, Pane, Session, Tmux};

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
}

#[derive(Clone, Debug, Serialize)]
pub struct DetectedSettings {
    pub settings: Settings,
    pub terminal_apps: Vec<InstalledTerminalApp>,
    /// Where new sessions start unless the user picks another directory.
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
        },
        terminal_apps,
        directory: detect_directory().display().to_string(),
        home_directory: std::env::var("HOME").ok(),
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct SessionList {
    pub sessions: Vec<SessionStatus>,
    /// Every running Claude Code session, in tmux or not.
    pub claude_code_sessions: Vec<ClaudeCodeSessionStatus>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ClaudeCodeSessionStatus {
    #[serde(flatten)]
    pub session: ClaudeCodeSession,
    /// Whether its tmux pane is on the listed tmux server. Sessions on
    /// another server (`tmux -L <name>`) have a pane id but aren't found.
    pub tmux_pane_found: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct SessionStatus {
    #[serde(flatten)]
    pub session: Session,
    pub windows: Vec<WindowStatus>,
}

#[derive(Clone, Debug, Serialize)]
pub struct WindowStatus {
    pub index: u32,
    pub name: String,
    pub active: bool,
    pub panes: Vec<PaneStatus>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PaneStatus {
    #[serde(flatten)]
    pub pane: Pane,
    pub state: PaneState,
    /// The name of the program in the foreground, like `claude` where tmux
    /// says `2.1.309`. The process group leader when there are several.
    pub foreground_program: Option<String>,
    /// The processes on the pane's tty. Processes without a tty, like the
    /// commands Claude Code runs, aren't included.
    pub processes: Vec<Process>,
    /// The Claude Code session running in the pane.
    pub claude_code: Option<ClaudeCodeSession>,
    /// The git status of the pane's directory, when it's in a repository.
    pub git: Option<GitStatus>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PaneState {
    /// A shell is in the foreground, waiting for a command.
    AtPrompt,
    /// Another program is in the foreground.
    Running,
    /// The pane's program exited.
    Exited,
    /// No foreground process was found.
    Unknown,
}

/// The user's tmux sessions, their windows and panes and what runs in each
/// pane, and their Claude Code sessions.
pub async fn list_sessions(tmux_path: &str) -> Result<SessionList, String> {
    let tmux = Tmux::new(tmux_path);
    let sessions = tmux.list_sessions().await?;
    let panes = tmux.list_panes().await?;
    let processes = process::list_processes().await?;
    let claude_code_sessions = claude_code::list_sessions(&processes).await;
    let git_statuses = get_git_statuses(&panes).await;

    let sessions = sessions
        .into_iter()
        .map(|session| {
            let session_panes = panes
                .iter()
                .filter(|pane| pane.session_name == session.name);
            let windows = group_into_windows(
                session_panes,
                &processes,
                &claude_code_sessions,
                &git_statuses,
            );
            SessionStatus { session, windows }
        })
        .collect();

    let claude_code_sessions = claude_code_sessions
        .into_iter()
        .map(|session| ClaudeCodeSessionStatus {
            tmux_pane_found: panes.iter().any(|pane| runs_in_pane(&session, pane)),
            session,
        })
        .collect();

    Ok(SessionList {
        sessions,
        claude_code_sessions,
    })
}

/// The git status of each pane's directory. Panes often share one, so each
/// directory is asked once, all at the same time.
async fn get_git_statuses(panes: &[Pane]) -> HashMap<String, GitStatus> {
    let mut directories: Vec<&str> = panes
        .iter()
        .map(|pane| pane.current_path.as_str())
        .collect();
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

/// `panes` come in window order from tmux.
fn group_into_windows<'a>(
    panes: impl Iterator<Item = &'a Pane>,
    processes: &[Process],
    claude_code_sessions: &[ClaudeCodeSession],
    git_statuses: &HashMap<String, GitStatus>,
) -> Vec<WindowStatus> {
    let mut windows: Vec<WindowStatus> = Vec::new();
    for pane in panes {
        let mut pane_status = get_pane_status(pane, processes, claude_code_sessions);
        pane_status.git = git_statuses.get(&pane.current_path).cloned();
        match windows.last_mut() {
            Some(window) if window.index == pane.window_index => window.panes.push(pane_status),
            _ => windows.push(WindowStatus {
                index: pane.window_index,
                name: pane.window_name.clone(),
                active: pane.window_active,
                panes: vec![pane_status],
            }),
        }
    }
    windows
}

fn get_pane_status(
    pane: &Pane,
    processes: &[Process],
    claude_code_sessions: &[ClaudeCodeSession],
) -> PaneStatus {
    let pane_processes: Vec<Process> = processes
        .iter()
        .filter(|process| process.tty.as_deref() == Some(pane.tty.as_str()))
        .cloned()
        .collect();
    let claude_code = claude_code_sessions
        .iter()
        .find(|session| runs_in_pane(session, pane))
        .cloned();

    let foreground_program = pane_processes
        .iter()
        .filter(|process| process.in_foreground)
        .min_by_key(|process| process.pid != process.process_group_id)
        .map(|process| process.name.clone());

    PaneStatus {
        state: get_pane_state(pane, &pane_processes),
        foreground_program,
        pane: pane.clone(),
        processes: pane_processes,
        claude_code,
        git: None,
    }
}

fn runs_in_pane(session: &ClaudeCodeSession, pane: &Pane) -> bool {
    session.tmux_pane_id.as_deref() == Some(pane.id.as_str())
        && session.tmux_session_name.as_deref() == Some(pane.session_name.as_str())
}

fn get_pane_state(pane: &Pane, pane_processes: &[Process]) -> PaneState {
    if pane.dead {
        return PaneState::Exited;
    }
    let mut foreground = pane_processes
        .iter()
        .filter(|process| process.in_foreground)
        .peekable();
    if foreground.peek().is_none() {
        return PaneState::Unknown;
    }
    if foreground.all(Process::is_shell) {
        PaneState::AtPrompt
    } else {
        PaneState::Running
    }
}

/// Creates a session running the user's shell in `directory`, types
/// `command` into it and opens it in the terminal app.
pub async fn create_session(
    settings: &Settings,
    name: &str,
    directory: &str,
    command: &str,
) -> Result<ShowOutcome, String> {
    tmux::validate_session_name(name)?;
    let directory = expand_home(directory.trim());
    if !directory.is_dir() {
        return Err(format!("{} is not a directory", directory.display()));
    }

    let tmux = Tmux::new(&settings.tmux_path);
    if tmux.has_session(name).await? {
        return Err(format!(
            "A session named {name} already exists, open it from the list"
        ));
    }

    tmux.create_session(name, &settings.shell, &directory.display().to_string())
        .await?;
    tmux.show_session_in_window_title(name).await?;
    if !command.trim().is_empty() {
        tmux.send_command(name, command.trim()).await?;
    }

    terminal_app::open_window(settings.terminal_app, &settings.tmux_path, name).await?;
    Ok(ShowOutcome::Opened)
}

/// Brings the session's terminal window to the front, or opens one. Sessions
/// the app didn't create get their window title set here so they can be
/// found next time.
pub async fn show_session_in_terminal_app(
    settings: &Settings,
    name: &str,
) -> Result<ShowOutcome, String> {
    tmux::validate_session_name(name)?;
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
