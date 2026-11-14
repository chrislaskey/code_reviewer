//! Guesses whether an agent that writes no status is busy or idle, from how
//! recently it wrote to a file it keeps open: written to in the last
//! `BUSY_WINDOW` means busy, else idle. A long tool call that writes nothing
//! reads as idle. Cursor's `agent` (`cursor_agent.rs`) and Codex
//! (`codex.rs`) are read this way.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::process::Command;

/// A bit more than the pages' refresh interval, so a refresh doesn't fall
/// between two writes.
const BUSY_WINDOW: Duration = Duration::from_secs(10);

/// What the last write to a file says of its writer.
#[derive(Clone, Debug, PartialEq)]
pub struct WriteStatus {
    /// `busy` or `idle`.
    pub status: String,
    /// The last write, in milliseconds since the Unix epoch.
    pub updated_at: Option<u64>,
}

/// The first file process `pid` has open that `is_wanted` accepts.
pub async fn find_open_file(pid: u32, is_wanted: impl Fn(&str) -> bool) -> Option<PathBuf> {
    find_in_open_files(&list_open_files(pid).await?, is_wanted)
}

/// The status of the agent writing `path`, or `None` when it can't be read.
pub async fn get_write_status(path: &Path) -> Option<WriteStatus> {
    let modified = tokio::fs::metadata(path).await.ok()?.modified().ok()?;
    Some(WriteStatus {
        status: status_of(modified, SystemTime::now()).to_string(),
        updated_at: milliseconds_since_epoch(modified),
    })
}

/// The output of `lsof -p <pid> -Fn`: one `n<path>` line per open file.
async fn list_open_files(pid: u32) -> Option<String> {
    let output = Command::new("lsof")
        .args(["-p", &pid.to_string(), "-Fn"])
        .output()
        .await
        .ok()?;
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn find_in_open_files(open_files: &str, is_wanted: impl Fn(&str) -> bool) -> Option<PathBuf> {
    open_files
        .lines()
        .filter_map(|line| line.strip_prefix('n'))
        .find(|path| is_wanted(path))
        .map(PathBuf::from)
}

fn status_of(last_write: SystemTime, now: SystemTime) -> &'static str {
    let age = now.duration_since(last_write).unwrap_or_default();
    if age <= BUSY_WINDOW { "busy" } else { "idle" }
}

fn milliseconds_since_epoch(time: SystemTime) -> Option<u64> {
    let duration = time.duration_since(UNIX_EPOCH).ok()?;
    u64::try_from(duration.as_millis()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_a_wanted_file_among_open_files() {
        let open_files = "p55812\nn/Users/me/code\nn/Users/me/notes.db\nn/dev/ttys000\n";
        assert_eq!(
            find_in_open_files(open_files, |path| path.ends_with(".db")),
            Some(PathBuf::from("/Users/me/notes.db"))
        );
        assert_eq!(find_in_open_files("p1\nn/dev/null\n", |_| false), None);
    }

    #[test]
    fn is_busy_only_after_a_recent_write() {
        let now = UNIX_EPOCH + Duration::from_secs(1_000);
        assert_eq!(status_of(now - Duration::from_secs(2), now), "busy");
        assert_eq!(status_of(now - Duration::from_secs(60), now), "idle");
        assert_eq!(status_of(now + Duration::from_secs(1), now), "busy");
    }
}
