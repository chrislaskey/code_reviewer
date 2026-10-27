//! Finds the user's terminal apps and shows a tmux session in one: focuses a
//! window already attached to it, or opens a new window attaching to it.
//!
//! Terminal and iTerm2 find windows by the tty of the session's tmux clients,
//! through AppleScript. Ghostty (AppleScript) and WezTerm (`wezterm cli`)
//! don't expose ttys, so they find windows by the title tmux gives them (see
//! `tmux::window_title`). kitty only allows finding windows with remote
//! control turned on in its config, so it always gets a new window.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tokio::process::Command;

use crate::tmux;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TerminalApp {
    Iterm2,
    Ghostty,
    Kitty,
    Wezterm,
    /// The Terminal app that comes with macOS.
    Terminal,
}

/// The order apps are picked in when several are installed. Terminal comes
/// with macOS, so it's the guess when none of the others is installed.
pub const DETECTION_ORDER: [TerminalApp; 5] = [
    TerminalApp::Iterm2,
    TerminalApp::Ghostty,
    TerminalApp::Kitty,
    TerminalApp::Wezterm,
    TerminalApp::Terminal,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ShowOutcome {
    /// An attached window was brought to the front.
    Focused,
    /// A new window attaching to the session was opened.
    Opened,
}

#[derive(Clone, Debug, Serialize)]
pub struct InstalledTerminalApp {
    pub id: TerminalApp,
    pub name: &'static str,
    /// Where the app is, or `None` when it isn't installed.
    pub path: Option<String>,
}

impl TerminalApp {
    pub fn name(self) -> &'static str {
        match self {
            Self::Iterm2 => "iTerm2",
            Self::Ghostty => "Ghostty",
            Self::Kitty => "kitty",
            Self::Wezterm => "WezTerm",
            Self::Terminal => "Terminal",
        }
    }

    fn bundle_id(self) -> &'static str {
        match self {
            Self::Iterm2 => "com.googlecode.iterm2",
            Self::Ghostty => "com.mitchellh.ghostty",
            Self::Kitty => "net.kovidgoyal.kitty",
            Self::Wezterm => "com.github.wez.wezterm",
            Self::Terminal => "com.apple.Terminal",
        }
    }

    fn bundle_file_name(self) -> &'static str {
        match self {
            Self::Iterm2 => "iTerm.app",
            Self::Ghostty => "Ghostty.app",
            Self::Kitty => "kitty.app",
            Self::Wezterm => "WezTerm.app",
            Self::Terminal => "Terminal.app",
        }
    }
}

pub async fn list_terminal_apps() -> Vec<InstalledTerminalApp> {
    let mut apps = Vec::new();
    for app in DETECTION_ORDER {
        apps.push(InstalledTerminalApp {
            id: app,
            name: app.name(),
            path: find_app_path(app)
                .await
                .map(|path| path.display().to_string()),
        });
    }
    apps
}

/// Focuses a window attached to the session when the app can find one,
/// otherwise opens a new one. `client_ttys` are the ttys of the terminals
/// attached to the session. `tmux_program` runs in the new window, so it
/// should be an absolute path.
pub async fn show_tmux_session(
    app: TerminalApp,
    tmux_program: &str,
    session_name: &str,
    client_ttys: &[String],
) -> Result<ShowOutcome, String> {
    if client_ttys.is_empty() {
        open_window(app, tmux_program, session_name).await?;
        return Ok(ShowOutcome::Opened);
    }
    if focus_window(app, &tmux::window_title(session_name), client_ttys).await? {
        return Ok(ShowOutcome::Focused);
    }
    open_window(app, tmux_program, session_name).await?;
    Ok(ShowOutcome::Opened)
}

/// Opens a new window attaching to the session and brings it to the front.
pub async fn open_window(
    app: TerminalApp,
    tmux_program: &str,
    session_name: &str,
) -> Result<(), String> {
    let target = tmux::exact_target(session_name);
    let attach_args = [tmux_program, "attach-session", "-t", &target];
    let attach_command = attach_args.map(shell_quote).join(" ");

    match app {
        TerminalApp::Ghostty => {
            run_applescript(&format!(
                r#"tell application id "{bundle_id}"
                     set config to new surface configuration
                     set command of config to {command}
                     new window with configuration config
                     activate
                   end tell"#,
                bundle_id = app.bundle_id(),
                command = applescript_string(&attach_command),
            ))
            .await?;
        }
        TerminalApp::Iterm2 => {
            run_applescript(&format!(
                r#"tell application id "{bundle_id}"
                     create window with default profile command {command}
                     activate
                   end tell"#,
                bundle_id = app.bundle_id(),
                command = applescript_string(&attach_command),
            ))
            .await?;
        }
        TerminalApp::Terminal => {
            // Terminal opens a window of its own when it starts, so when it
            // isn't running the command goes into that window.
            run_applescript(&format!(
                r#"set wasRunning to application id "{bundle_id}" is running
                   tell application id "{bundle_id}"
                     if wasRunning then
                       do script {command}
                     else
                       activate
                       repeat 50 times
                         if (count of windows) > 0 then exit repeat
                         delay 0.1
                       end repeat
                       if (count of windows) > 0 then
                         do script {command} in window 1
                       else
                         do script {command}
                       end if
                     end if
                     activate
                   end tell"#,
                bundle_id = app.bundle_id(),
                command = applescript_string(&attach_command),
            ))
            .await?;
        }
        TerminalApp::Kitty => {
            open_new_instance(app, &attach_args).await?;
        }
        TerminalApp::Wezterm => {
            let mut args = vec!["start", "--"];
            args.extend(attach_args);
            open_new_instance(app, &args).await?;
        }
    }
    Ok(())
}

/// Brings the window on one of `client_ttys`, or titled `title`, to the
/// front. `Ok(false)` means the app isn't running, has no such window or
/// can't be asked.
async fn focus_window(
    app: TerminalApp,
    title: &str,
    client_ttys: &[String],
) -> Result<bool, String> {
    let ttys = applescript_list(client_ttys);
    match app {
        TerminalApp::Ghostty => {
            let found = run_applescript(&format!(
                r#"if application id "{bundle_id}" is not running then return "not-found"
                   tell application id "{bundle_id}"
                     set matches to every terminal whose name is {title}
                     if (count of matches) is 0 then return "not-found"
                     focus (item 1 of matches)
                     activate
                     return "focused"
                   end tell"#,
                bundle_id = app.bundle_id(),
                title = applescript_string(title),
            ))
            .await?;
            Ok(found == "focused")
        }
        TerminalApp::Iterm2 => {
            let found = run_applescript(&format!(
                r#"if application id "{bundle_id}" is not running then return "not-found"
                   tell application id "{bundle_id}"
                     repeat with w in windows
                       repeat with t in tabs of w
                         repeat with s in sessions of t
                           if (tty of s) is in {ttys} then
                             select w
                             select t
                             select s
                             activate
                             return "focused"
                           end if
                         end repeat
                       end repeat
                     end repeat
                     return "not-found"
                   end tell"#,
                bundle_id = app.bundle_id(),
            ))
            .await?;
            Ok(found == "focused")
        }
        TerminalApp::Terminal => {
            let found = run_applescript(&format!(
                r#"if application id "{bundle_id}" is not running then return "not-found"
                   tell application id "{bundle_id}"
                     repeat with w in windows
                       repeat with t in tabs of w
                         if (tty of t) is in {ttys} then
                           set selected tab of w to t
                           set index of w to 1
                           activate
                           return "focused"
                         end if
                       end repeat
                     end repeat
                     return "not-found"
                   end tell"#,
                bundle_id = app.bundle_id(),
            ))
            .await?;
            Ok(found == "focused")
        }
        TerminalApp::Wezterm => focus_wezterm_pane(title).await,
        TerminalApp::Kitty => Ok(false),
    }
}

/// `wezterm cli` talks to the running WezTerm, so it fails when there is
/// none. When several WezTerm processes run it only sees the newest.
async fn focus_wezterm_pane(title: &str) -> Result<bool, String> {
    #[derive(Deserialize)]
    struct Pane {
        pane_id: u64,
        title: String,
    }

    let Some(app_path) = find_app_path(TerminalApp::Wezterm).await else {
        return Ok(false);
    };
    let cli = app_path.join("Contents/MacOS/wezterm");
    let Ok(output) = Command::new(&cli)
        .args(["cli", "list", "--format", "json"])
        .output()
        .await
    else {
        return Ok(false);
    };
    if !output.status.success() {
        return Ok(false);
    }
    let panes: Vec<Pane> = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("Could not read wezterm cli list: {error}"))?;
    let Some(pane) = panes.into_iter().find(|pane| pane.title == title) else {
        return Ok(false);
    };

    run_checked(Command::new(&cli).args([
        "cli",
        "activate-pane",
        "--pane-id",
        &pane.pane_id.to_string(),
    ]))
    .await?;
    run_checked(Command::new("open").args(["-b", TerminalApp::Wezterm.bundle_id()])).await?;
    Ok(true)
}

/// Starts another instance of the app with `args`. Each one opens its own
/// window.
async fn open_new_instance(app: TerminalApp, args: &[&str]) -> Result<(), String> {
    run_checked(
        Command::new("open")
            .args(["-n", "-b", app.bundle_id(), "--args"])
            .args(args),
    )
    .await
}

async fn find_app_path(app: TerminalApp) -> Option<PathBuf> {
    let query = format!("kMDItemCFBundleIdentifier == '{}'", app.bundle_id());
    if let Ok(output) = Command::new("mdfind").arg(&query).output().await {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if let Some(path) = stdout.lines().map(Path::new).find(|path| path.exists()) {
            return Some(path.to_path_buf());
        }
    }

    let home_applications = std::env::var("HOME")
        .map(|home| Path::new(&home).join("Applications"))
        .ok();
    [
        Some(PathBuf::from("/Applications")),
        Some(PathBuf::from("/System/Applications/Utilities")),
        home_applications,
    ]
    .into_iter()
    .flatten()
    .map(|directory| directory.join(app.bundle_file_name()))
    .find(|path| path.exists())
}

/// Runs an AppleScript and returns what it returned. macOS asks the user
/// once before the app may control another app (Automation permission).
async fn run_applescript(script: &str) -> Result<String, String> {
    let output = Command::new("osascript")
        .args(["-e", script])
        .output()
        .await
        .map_err(|error| format!("Could not run osascript: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "AppleScript failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

async fn run_checked(command: &mut Command) -> Result<(), String> {
    let output = command.output().await.map_err(|error| {
        format!(
            "Could not run {:?}: {error}",
            command.as_std().get_program()
        )
    })?;
    if output.status.success() {
        return Ok(());
    }
    Err(format!(
        "{:?} failed: {}",
        command.as_std().get_program(),
        String::from_utf8_lossy(&output.stderr).trim()
    ))
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}

fn applescript_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', r"\\").replace('"', "\\\""))
}

fn applescript_list(values: &[String]) -> String {
    let items: Vec<String> = values
        .iter()
        .map(|value| applescript_string(value))
        .collect();
    format!("{{{}}}", items.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_for_the_shell() {
        assert_eq!(shell_quote("/opt/my tmux"), "'/opt/my tmux'");
        assert_eq!(shell_quote("it's"), r"'it'\''s'");
    }

    #[test]
    fn quotes_for_applescript() {
        assert_eq!(
            applescript_string(r#"say "hi" \o/"#),
            r#""say \"hi\" \\o/""#
        );
        assert_eq!(
            applescript_list(&["/dev/ttys001".into(), "/dev/ttys002".into()]),
            r#"{"/dev/ttys001", "/dev/ttys002"}"#
        );
    }
}
