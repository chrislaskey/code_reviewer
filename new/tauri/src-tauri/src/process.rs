//! Lists the processes on the machine with `ps`, to tell what runs in a tmux
//! pane.

use serde::Serialize;
use tokio::process::Command;

/// Programs that mean a pane is sitting at a prompt when they are in the
/// foreground.
const SHELLS: [&str; 8] = ["sh", "bash", "zsh", "fish", "dash", "ksh", "tcsh", "nu"];

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Process {
    pub pid: u32,
    pub parent_pid: u32,
    pub process_group_id: u32,
    /// The tty's foreground process group.
    pub foreground_process_group_id: u32,
    /// Whether the process is in its tty's foreground process group, the
    /// one that gets keyboard input.
    pub in_foreground: bool,
    /// Like `ttys035`, or `None` for processes without a terminal.
    pub tty: Option<String>,
    /// The `ps` state code, like `S+` or `R`.
    pub state_code: String,
    pub state: ProcessState,
    pub cpu_percent: f32,
    /// A readable program name, like `zsh`, `nvim` or `claude`.
    pub name: String,
    /// The program as `ps` shows it, often a full path.
    pub command: String,
}

/// The first letter of the `ps` state code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ProcessState {
    Running,
    Sleeping,
    /// Sleeping for more than about 20 seconds.
    Idle,
    Stopped,
    /// Waiting on the disk or another resource it can't be interrupted from.
    Waiting,
    Zombie,
    Unknown,
}

impl Process {
    pub fn is_shell(&self) -> bool {
        SHELLS.contains(&self.name.as_str())
    }

    pub fn is_claude_code(&self) -> bool {
        self.name == "claude"
    }
}

pub async fn list_processes() -> Result<Vec<Process>, String> {
    let output = Command::new("ps")
        .args(["-A", "-o", "pid=,ppid=,pgid=,tpgid=,tty=,stat=,%cpu=,comm="])
        .output()
        .await
        .map_err(|error| format!("Could not run ps: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "ps failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(parse_process)
        .collect())
}

fn parse_process(line: &str) -> Option<Process> {
    let mut fields = line.split_whitespace();
    let pid = fields.next()?.parse().ok()?;
    let parent_pid = fields.next()?.parse().ok()?;
    let process_group_id = fields.next()?.parse().ok()?;
    let foreground_process_group_id = fields.next()?.parse().ok()?;
    let tty = Some(fields.next()?.to_string()).filter(|tty| tty != "??");
    let state_code = fields.next()?.to_string();
    let cpu_percent = fields.next()?.parse().ok()?;
    // The command is last and may contain spaces.
    let command = fields.collect::<Vec<_>>().join(" ");
    if command.is_empty() {
        return None;
    }

    Some(Process {
        pid,
        parent_pid,
        process_group_id,
        foreground_process_group_id,
        in_foreground: process_group_id == foreground_process_group_id,
        tty,
        state: parse_state(&state_code),
        state_code,
        cpu_percent,
        name: program_name(&command),
        command,
    })
}

fn parse_state(state_code: &str) -> ProcessState {
    match state_code.chars().next() {
        Some('R') => ProcessState::Running,
        Some('S') => ProcessState::Sleeping,
        Some('I') => ProcessState::Idle,
        Some('T') => ProcessState::Stopped,
        Some('U' | 'D') => ProcessState::Waiting,
        Some('Z') => ProcessState::Zombie,
        _ => ProcessState::Unknown,
    }
}

/// `-zsh` (a login shell) is `zsh`, and Claude Code's binary, named after
/// its version like `~/.local/share/claude/versions/2.1.309`, is `claude`.
fn program_name(command: &str) -> String {
    if command.contains("/claude/versions/") {
        return "claude".into();
    }
    let base_name = command.rsplit('/').next().unwrap_or(command);
    base_name.trim_start_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ps_lines() {
        let process = parse_process(
            "39100 45306 39100 39100 ttys035  Ss+   17.0 /Users/me/.local/share/claude/versions/2.1.309",
        )
        .expect("the line parses");
        assert_eq!(process.pid, 39100);
        assert_eq!(process.tty.as_deref(), Some("ttys035"));
        assert_eq!(process.state, ProcessState::Sleeping);
        assert_eq!(process.cpu_percent, 17.0);
        assert_eq!(process.name, "claude");
        assert!(process.in_foreground);
        assert!(process.is_claude_code());

        let shell = parse_process("45380 45306 45380 96377 ttys008  Ss     0.0 -zsh")
            .expect("the line parses");
        assert_eq!(shell.name, "zsh");
        assert!(shell.is_shell());
        assert!(!shell.in_foreground);

        let daemon = parse_process("  1     0     1     0 ??       Ss     0.3 /sbin/launchd")
            .expect("the line parses");
        assert_eq!(daemon.tty, None);
        assert_eq!(daemon.name, "launchd");
    }

    #[test]
    fn keeps_spaces_in_commands() {
        let process =
            parse_process("5 1 5 0 ?? R 1.5 /Applications/My App.app/Contents/MacOS/My App")
                .expect("the line parses");
        assert_eq!(process.state, ProcessState::Running);
        assert_eq!(process.name, "My App");
    }
}
