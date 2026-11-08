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

/// How long to wait between pasting a prompt and pressing Enter.
const PASTE_SETTLE_TIME: std::time::Duration = std::time::Duration::from_millis(150);

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
    /// Finder. `environment` is set in the session's environment, so the
    /// shell and everything started in the session see it (tmux 3.2 or
    /// later), and `get_environment_variable` reads it back.
    pub async fn create_session(
        &self,
        name: &str,
        shell: &str,
        working_directory: &str,
        environment: &[(&str, &str)],
    ) -> Result<(), String> {
        let assignments: Vec<String> = environment
            .iter()
            .map(|(variable, value)| format!("{variable}={value}"))
            .collect();
        let mut args = vec!["new-session", "-d", "-s", name, "-c", working_directory];
        for assignment in &assignments {
            args.extend(["-e", assignment]);
        }
        args.extend(["--", shell, "-l"]);
        self.run_checked(&args).await
    }

    /// The value of `variable` in the session's environment, `None` when
    /// the session has no such variable (or has unset it).
    pub async fn get_environment_variable(
        &self,
        name: &str,
        variable: &str,
    ) -> Result<Option<String>, String> {
        let output = self
            .run(&["show-environment", "-t", &exact_target(name), variable])
            .await?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("unknown variable") {
                return Ok(None);
            }
            return Err(format!("tmux show-environment failed: {}", stderr.trim()));
        }
        let line = String::from_utf8_lossy(&output.stdout);
        Ok(line
            .trim_end()
            .strip_prefix(variable)
            .and_then(|rest| rest.strip_prefix('='))
            .map(str::to_string))
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

    /// Pastes `prompt` into the pane `pane_id` (like `%60`) and presses
    /// Enter. A bracketed paste, so an agent reads a prompt of several lines
    /// as one, rather than each newline as Enter.
    pub async fn send_prompt_to_pane(&self, pane_id: &str, prompt: &str) -> Result<(), String> {
        validate_pane_id(pane_id)?;
        let buffer = format!("code-reviewer-prompt-{}", pane_id.trim_start_matches('%'));
        self.run_checked(&["set-buffer", "-b", &buffer, "--", prompt])
            .await?;
        self.run_checked(&["paste-buffer", "-p", "-d", "-b", &buffer, "-t", pane_id])
            .await?;
        // An agent can read an Enter that comes with the paste as part of it.
        tokio::time::sleep(PASTE_SETTLE_TIME).await;
        self.run_checked(&["send-keys", "-t", pane_id, "Enter"])
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
        for (name, _) in std::env::vars_os() {
            if name.to_str().is_some_and(is_app_build_variable) {
                command.env_remove(name);
            }
        }
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

/// Whether `name` is one of the variables `cargo tauri dev` sets for the
/// app's own build and run, like `CARGO_PKG_NAME`, `TAURI_CONFIG` and the
/// app's `PORT`. A tmux command that starts the server copies its
/// environment into every session, so these are kept from tmux; a session's
/// login shell sets the user's own again.
fn is_app_build_variable(name: &str) -> bool {
    name == "CARGO"
        || name.starts_with("CARGO_")
        || name.starts_with("TAURI_")
        || name == "OUT_DIR"
        || name == "DYLD_FALLBACK_LIBRARY_PATH"
        || name == "PORT"
}

/// Pane ids are `%` and a number, like `%60`.
fn validate_pane_id(pane_id: &str) -> Result<(), String> {
    let is_valid = pane_id
        .strip_prefix('%')
        .is_some_and(|number| !number.is_empty() && number.chars().all(|c| c.is_ascii_digit()));
    if is_valid {
        Ok(())
    } else {
        Err(format!("{pane_id} is not a tmux pane id"))
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

    #[test]
    fn validates_pane_ids() {
        assert!(validate_pane_id("%60").is_ok());
        assert!(validate_pane_id("%").is_err());
        assert!(validate_pane_id("=session:").is_err());
    }

    #[tokio::test]
    async fn sends_a_prompt_to_a_pane() {
        if !tmux_installed().await {
            return;
        }
        let server = TestServer::start("prompt");
        let tmux = &server.tmux;
        let name = "example";
        tmux.create_session(name, "/bin/sh", "/tmp", &[])
            .await
            .expect("session is created");
        let panes = tmux.list_panes().await.expect("panes are listed");

        tmux.send_prompt_to_pane(&panes[0].id, "echo prompt-$((40 + 2))")
            .await
            .expect("prompt is sent");

        let mut screen = String::new();
        for _ in 0..50 {
            screen = tmux.capture_pane(name).await;
            if screen.contains("prompt-42") {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert!(screen.contains("prompt-42"), "screen was: {screen}");
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

    #[test]
    fn app_build_variables_are_the_cargo_and_tauri_ones() {
        for name in ["CARGO", "CARGO_PKG_NAME", "TAURI_CONFIG", "OUT_DIR", "PORT"] {
            assert!(is_app_build_variable(name), "{name}");
        }
        for name in ["HOME", "PATH", "CARGOX", "EXAMPLE_PORT", "SHELL"] {
            assert!(!is_app_build_variable(name), "{name}");
        }
    }

    #[tokio::test]
    async fn keeps_the_app_build_variables_out_of_a_new_server() {
        if !tmux_installed().await {
            return;
        }
        let server = TestServer::start("build-variables");
        let tmux = &server.tmux;
        tmux.create_session("example", "/bin/sh", "/tmp", &[])
            .await
            .expect("session is created");

        // `cargo test` sets CARGO_PKG_NAME for this process, as `cargo tauri dev` does for the app.
        let global = tmux
            .run(&["show-environment", "-g"])
            .await
            .expect("show-environment runs");
        let global = String::from_utf8_lossy(&global.stdout);

        assert!(std::env::var_os("CARGO_PKG_NAME").is_some());
        assert!(!global.contains("CARGO_PKG_NAME="), "global was: {global}");
        assert!(global.contains("HOME="), "global was: {global}");
    }

    #[tokio::test]
    async fn creates_a_session_and_sends_it_a_command() {
        if !tmux_installed().await {
            return;
        }
        let server = TestServer::start("create");
        let tmux = &server.tmux;
        let name = "example";

        tmux.create_session(name, "/bin/sh", "/tmp", &[("EXAMPLE_PORT", "4242")])
            .await
            .expect("session is created");
        tmux.show_session_in_window_title(name)
            .await
            .expect("title is set");
        tmux.send_command(name, "echo sent-$EXAMPLE_PORT")
            .await
            .expect("command is sent");

        let mut screen = String::new();
        for _ in 0..50 {
            screen = tmux.capture_pane(name).await;
            if screen.contains("sent-4242") {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        let port = tmux.get_environment_variable(name, "EXAMPLE_PORT").await;
        let missing = tmux.get_environment_variable(name, "EXAMPLE_MISSING").await;
        let sessions = tmux.list_sessions().await;
        let panes = tmux.list_panes().await;
        let client_ttys = tmux.list_client_ttys(name).await;
        let exists = tmux.has_session(name).await;
        let prefix_matches = tmux.has_session("exam").await;
        tmux.kill_session(name).await.expect("session is killed");
        let exists_after_kill = tmux.has_session(name).await;

        assert!(screen.contains("sent-4242"), "screen was: {screen}");
        assert_eq!(port, Ok(Some("4242".to_string())));
        assert_eq!(missing, Ok(None));
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
