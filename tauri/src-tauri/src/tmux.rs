//! Runs the user's tmux: lists sessions and panes, creates sessions and types
//! commands into them.
//!
//! The app shares the user's tmux server, so sessions it creates show up in
//! `tmux ls` and the user can attach to them from any terminal.

use std::process::Output;

use serde::Serialize;
use tokio::process::Command;

/// Separates the fields of tmux's list output. tmux prints other control
/// characters escaped, like `\037`. A line with a tab in a name has too many
/// fields and is skipped.
const FIELD_SEPARATOR: &str = "\t";
const SESSION_FIELDS: [&str; 4] = [
    "#{session_windows}",
    "#{session_attached}",
    "#{pane_current_command}",
    "#{session_name}",
];
const PANE_FIELDS: [&str; 12] = [
    "#{session_name}",
    "#{window_index}",
    "#{window_name}",
    "#{window_active}",
    "#{pane_id}",
    "#{pane_index}",
    "#{pane_active}",
    "#{pane_pid}",
    "#{pane_tty}",
    "#{pane_dead}",
    "#{pane_current_command}",
    "#{pane_current_path}",
];

/// The terminal window title tmux sets for a session (see
/// `show_session_in_window_title`), which terminal apps find windows by.
pub fn window_title(session_name: &str) -> String {
    format!("tmux:{session_name}")
}

/// Session names the app accepts. tmux itself forbids `.` and `:`, and
/// keeping to these characters means names never need quoting.
pub fn validate_session_name(name: &str) -> Result<(), String> {
    let allowed = |c: char| c.is_ascii_alphanumeric() || c == '-' || c == '_';
    if name.is_empty() || name.len() > 64 || !name.chars().all(allowed) {
        return Err("Session names may only use letters, digits, - and _ (up to 64)".into());
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Session {
    pub name: String,
    pub window_count: u32,
    /// How many terminals are attached to the session right now.
    pub attached_clients: u32,
    /// What runs in the session's active pane, like `zsh` or `claude`.
    pub current_command: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Pane {
    pub session_name: String,
    pub window_index: u32,
    pub window_name: String,
    /// Whether this is the session's current window.
    pub window_active: bool,
    /// Like `%60`, unique on the tmux server.
    pub id: String,
    pub index: u32,
    /// Whether this is the window's current pane.
    pub active: bool,
    /// The pane's first process, usually a shell.
    pub pid: u32,
    /// Like `ttys035`, as `ps` shows it.
    pub tty: String,
    /// Whether the pane's program has exited (with `remain-on-exit`).
    pub dead: bool,
    /// The program in the foreground, as tmux names it.
    pub current_command: String,
    pub current_path: String,
}

#[derive(Clone, Debug)]
pub struct Tmux {
    program: String,
    /// A separate tmux server (`tmux -S <path>`), so tests don't touch the
    /// user's sessions.
    socket_path: Option<std::path::PathBuf>,
}

impl Tmux {
    pub fn new(program: impl Into<String>) -> Self {
        Self {
            program: program.into(),
            socket_path: None,
        }
    }

    pub async fn list_sessions(&self) -> Result<Vec<Session>, String> {
        let lines = self.list("list-sessions", &[], &SESSION_FIELDS).await?;
        Ok(lines
            .iter()
            .filter_map(|line| parse_session(line))
            .collect())
    }

    /// Every pane of every session.
    pub async fn list_panes(&self) -> Result<Vec<Pane>, String> {
        let lines = self.list("list-panes", &["-a"], &PANE_FIELDS).await?;
        Ok(lines.iter().filter_map(|line| parse_pane(line)).collect())
    }

    /// Runs a tmux list command printing `fields`, one line per item. No
    /// tmux server running means nothing to list.
    async fn list(
        &self,
        command: &str,
        args: &[&str],
        fields: &[&str],
    ) -> Result<Vec<String>, String> {
        let format = fields.join(FIELD_SEPARATOR);
        let mut all_args = vec![command];
        all_args.extend(args);
        all_args.extend(["-F", &format]);

        let output = self.run(&all_args).await?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("no server running") || stderr.contains("error connecting") {
                return Ok(Vec::new());
            }
            return Err(format!("tmux {command} failed: {}", stderr.trim()));
        }
        Ok(String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::to_string)
            .collect())
    }

    pub async fn has_session(&self, name: &str) -> Result<bool, String> {
        let output = self
            .run(&["has-session", "-t", &exact_target(name)])
            .await?;
        Ok(output.status.success())
    }

    /// The ttys of the terminals attached to the session.
    pub async fn list_client_ttys(&self, name: &str) -> Result<Vec<String>, String> {
        let output = self
            .run(&[
                "list-clients",
                "-t",
                &exact_target(name),
                "-F",
                "#{client_tty}",
            ])
            .await?;
        if !output.status.success() {
            return Err(format!(
                "tmux list-clients failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::to_string)
            .collect())
    }

    /// Starts a detached session running `shell` as a login shell, so it
    /// gets the user's full environment even when the app was launched from
    /// Finder.
    pub async fn create_session(
        &self,
        name: &str,
        shell: &str,
        working_directory: &str,
    ) -> Result<(), String> {
        self.run_checked(&[
            "new-session",
            "-d",
            "-s",
            name,
            "-c",
            working_directory,
            "--",
            shell,
            "-l",
        ])
        .await
    }

    /// Makes terminals attached to the session show `window_title` as their
    /// title. Only this session's options change, not the user's other
    /// sessions.
    pub async fn show_session_in_window_title(&self, name: &str) -> Result<(), String> {
        let target = exact_target(name);
        self.run_checked(&["set-option", "-t", &target, "set-titles", "on"])
            .await?;
        self.run_checked(&[
            "set-option",
            "-t",
            &target,
            "set-titles-string",
            &window_title("#S"),
        ])
        .await
    }

    /// Ends the session and everything running in it.
    pub async fn kill_session(&self, name: &str) -> Result<(), String> {
        self.run_checked(&["kill-session", "-t", &exact_target(name)])
            .await
    }

    /// Types `command` into the session's active pane and presses Enter. The
    /// shell reads it once it has started.
    pub async fn send_command(&self, name: &str, command: &str) -> Result<(), String> {
        let target = exact_target(name);
        self.run_checked(&["send-keys", "-t", &target, "-l", command])
            .await?;
        self.run_checked(&["send-keys", "-t", &target, "Enter"])
            .await
    }

    async fn run_checked(&self, args: &[&str]) -> Result<(), String> {
        let output = self.run(args).await?;
        if output.status.success() {
            return Ok(());
        }
        Err(format!(
            "tmux {} failed: {}",
            args.first().unwrap_or(&""),
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }

    async fn run(&self, args: &[&str]) -> Result<Output, String> {
        let mut command = Command::new(&self.program);
        if let Some(socket_path) = &self.socket_path {
            command.arg("-S").arg(socket_path);
        }
        command
            .args(args)
            .output()
            .await
            .map_err(|error| format!("Could not run {}: {error}", self.program))
    }
}

/// A target matching the session named exactly `name`, not every session
/// starting with it. The trailing `:` makes it valid where tmux expects a
/// pane too, like `set-option` and `send-keys`.
pub fn exact_target(name: &str) -> String {
    format!("={name}:")
}

fn parse_session(line: &str) -> Option<Session> {
    let [window_count, attached_clients, current_command, name] = split_fields(line)?;
    Some(Session {
        name: name.to_string(),
        window_count: window_count.parse().ok()?,
        attached_clients: attached_clients.parse().ok()?,
        current_command: current_command.to_string(),
    })
}

fn parse_pane(line: &str) -> Option<Pane> {
    let [
        session_name,
        window_index,
        window_name,
        window_active,
        id,
        index,
        active,
        pid,
        tty,
        dead,
        current_command,
        current_path,
    ] = split_fields(line)?;
    Some(Pane {
        session_name: session_name.to_string(),
        window_index: window_index.parse().ok()?,
        window_name: window_name.to_string(),
        window_active: window_active == "1",
        id: id.to_string(),
        index: index.parse().ok()?,
        active: active == "1",
        pid: pid.parse().ok()?,
        tty: tty.trim_start_matches("/dev/").to_string(),
        dead: dead == "1",
        current_command: current_command.to_string(),
        current_path: current_path.to_string(),
    })
}

fn split_fields<const N: usize>(line: &str) -> Option<[&str; N]> {
    line.split(FIELD_SEPARATOR)
        .collect::<Vec<_>>()
        .try_into()
        .ok()
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    /// A tmux server of its own, stopped and its socket removed when the
    /// test ends, even when it fails.
    struct TestServer {
        tmux: Tmux,
    }

    impl TestServer {
        fn start(name: &str) -> Self {
            let socket_path = std::env::temp_dir().join(format!(
                "code-reviewer-test-{name}-{}.sock",
                std::process::id()
            ));
            Self {
                tmux: Tmux {
                    program: "tmux".into(),
                    socket_path: Some(socket_path),
                },
            }
        }
    }

    impl Drop for TestServer {
        fn drop(&mut self) {
            let Some(socket_path) = &self.tmux.socket_path else {
                return;
            };
            let _ = std::process::Command::new(&self.tmux.program)
                .arg("-S")
                .arg(socket_path)
                .arg("kill-server")
                .output();
            let _ = std::fs::remove_file(socket_path);
        }
    }

    impl Tmux {
        async fn capture_pane(&self, name: &str) -> String {
            let output = self
                .run(&["capture-pane", "-p", "-t", &exact_target(name)])
                .await
                .expect("capture-pane runs");
            String::from_utf8_lossy(&output.stdout).into_owned()
        }
    }

    async fn tmux_installed() -> bool {
        Command::new("tmux").arg("-V").output().await.is_ok()
    }

    #[test]
    fn validates_session_names() {
        assert!(validate_session_name("code-reviewer_2").is_ok());
        assert!(validate_session_name("").is_err());
        assert!(validate_session_name("has.dot").is_err());
        assert!(validate_session_name("has:colon").is_err());
        assert!(validate_session_name("has space").is_err());
        assert!(validate_session_name(&"a".repeat(65)).is_err());
    }

    #[test]
    fn parses_list_sessions_lines() {
        assert_eq!(
            parse_session("2\t1\tclaude\tmy session"),
            Some(Session {
                name: "my session".into(),
                window_count: 2,
                attached_clients: 1,
                current_command: "claude".into(),
            })
        );
        assert_eq!(parse_session("not a session"), None);
    }

    #[test]
    fn parses_list_panes_lines() {
        let line = [
            "my-session",
            "1",
            "editor",
            "1",
            "%60",
            "0",
            "1",
            "39100",
            "/dev/ttys035",
            "0",
            "2.1.309",
            "/code",
        ]
        .join(FIELD_SEPARATOR);
        let pane = parse_pane(&line).expect("the line parses");

        assert_eq!(pane.window_name, "editor");
        assert_eq!(pane.id, "%60");
        assert_eq!(pane.tty, "ttys035");
        assert!(pane.active && pane.window_active && !pane.dead);
        assert_eq!(parse_pane("too\tfew"), None);
    }

    #[tokio::test]
    async fn lists_no_sessions_without_a_server() {
        if !tmux_installed().await {
            return;
        }
        let server = TestServer::start("empty");
        let tmux = &server.tmux;
        assert_eq!(tmux.list_sessions().await, Ok(Vec::new()));
    }

    #[tokio::test]
    async fn creates_a_session_and_sends_it_a_command() {
        if !tmux_installed().await {
            return;
        }
        let server = TestServer::start("create");
        let tmux = &server.tmux;
        let name = "example";

        tmux.create_session(name, "/bin/sh", "/tmp")
            .await
            .expect("session is created");
        tmux.show_session_in_window_title(name)
            .await
            .expect("title is set");
        tmux.send_command(name, "echo sent-$((40 + 2))")
            .await
            .expect("command is sent");

        let mut screen = String::new();
        for _ in 0..50 {
            screen = tmux.capture_pane(name).await;
            if screen.contains("sent-42") {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        let sessions = tmux.list_sessions().await;
        let panes = tmux.list_panes().await;
        let client_ttys = tmux.list_client_ttys(name).await;
        let exists = tmux.has_session(name).await;
        let prefix_matches = tmux.has_session("exam").await;
        tmux.kill_session(name).await.expect("session is killed");
        let exists_after_kill = tmux.has_session(name).await;

        assert!(screen.contains("sent-42"), "screen was: {screen}");
        assert_eq!(exists, Ok(true));
        assert_eq!(prefix_matches, Ok(false));
        assert_eq!(exists_after_kill, Ok(false));
        assert_eq!(client_ttys, Ok(Vec::new()));
        let sessions = sessions.expect("sessions are listed");
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].name, name);
        assert_eq!(sessions[0].attached_clients, 0);
        let panes = panes.expect("panes are listed");
        assert_eq!(panes.len(), 1);
        assert_eq!(panes[0].session_name, name);
        assert_eq!(panes[0].current_path, "/private/tmp");
    }
}
