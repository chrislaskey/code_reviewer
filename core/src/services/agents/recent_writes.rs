//! Guesses whether an agent that writes no status is busy or idle, from how
//! recently it wrote to a file it keeps open: written to in the last
//! `BUSY_WINDOW` means busy, else idle. A long tool call that writes nothing
//! reads as idle. Cursor's `agent` (`cursor_agent.rs`) and Codex
//! (`codex.rs`) are read this way.
//!
//! Which file a process writes is found with `lsof`, and kept in an
//! `OpenFileCache` so it isn't run on every refresh.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::process::Command;

/// A bit more than the pages' refresh interval, so a refresh doesn't fall
/// between two writes.
const BUSY_WINDOW: Duration = Duration::from_secs(10);
/// How long a file found with `lsof`, or finding none, is trusted before
/// looking again, unless the file is still being written. An agent switches
/// files when it starts a new chat.
const LOOK_AGAIN_AFTER: Duration = Duration::from_secs(15);

/// What the last write to a file says of its writer.
#[derive(Clone, Debug, PartialEq)]
pub struct WriteStatus {
    /// `busy` or `idle`.
    pub status: String,
    /// The last write, in milliseconds since the Unix epoch.
    pub updated_at: Option<u64>,
}

/// The file each agent process was last found to have open, by pid.
#[derive(Default)]
pub struct OpenFileCache {
    found_files: Mutex<HashMap<u32, FoundFile>>,
}

#[derive(Clone)]
struct FoundFile {
    path: Option<PathBuf>,
    found_at: SystemTime,
}

impl OpenFileCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// The first file process `pid` has open that `is_wanted` accepts, as
    /// last found, looking again with `lsof` once that's out of date.
    pub async fn find_open_file(
        &self,
        pid: u32,
        is_wanted: impl Fn(&str) -> bool,
    ) -> Option<PathBuf> {
        if let Some(found) = self.get(pid) {
            let last_write = match &found.path {
                Some(path) => last_write_of(path).await,
                None => None,
            };
            if is_up_to_date(&found, last_write, SystemTime::now()) {
                return found.path;
            }
        }
        let path = find_in_open_files(&list_open_files(pid).await?, is_wanted);
        self.keep(pid, path.clone());
        path
    }

    /// Drops the files of processes not in `running_pids`.
    pub fn forget_exited_processes(&self, running_pids: &[u32]) {
        if let Ok(mut found_files) = self.found_files.lock() {
            found_files.retain(|pid, _| running_pids.contains(pid));
        }
    }

    fn get(&self, pid: u32) -> Option<FoundFile> {
        self.found_files.lock().ok()?.get(&pid).cloned()
    }

    fn keep(&self, pid: u32, path: Option<PathBuf>) {
        if let Ok(mut found_files) = self.found_files.lock() {
            let found_at = SystemTime::now();
            found_files.insert(pid, FoundFile { path, found_at });
        }
    }
}

/// The status of the agent writing `path`, or `None` when it can't be read.
pub async fn get_write_status(path: &Path) -> Option<WriteStatus> {
    let modified = last_write_of(path).await?;
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

async fn last_write_of(path: &Path) -> Option<SystemTime> {
    tokio::fs::metadata(path).await.ok()?.modified().ok()
}

/// A found file is trusted while it's being written, or for
/// `LOOK_AGAIN_AFTER` after it was found; a file since removed never is.
fn is_up_to_date(found: &FoundFile, last_write: Option<SystemTime>, now: SystemTime) -> bool {
    if found.path.is_some() && last_write.is_none() {
        return false;
    }
    let age = |time: SystemTime| now.duration_since(time).unwrap_or_default();
    age(found.found_at) < LOOK_AGAIN_AFTER
        || last_write.is_some_and(|time| age(time) <= BUSY_WINDOW)
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
    fn trusts_a_found_file_while_it_is_written_or_recently_found() {
        let now = UNIX_EPOCH + Duration::from_secs(1_000);
        let found = |path: Option<&str>, seconds_ago: u64| FoundFile {
            path: path.map(PathBuf::from),
            found_at: now - Duration::from_secs(seconds_ago),
        };
        let written = |seconds_ago: u64| Some(now - Duration::from_secs(seconds_ago));

        assert!(is_up_to_date(&found(Some("/a"), 5), written(60), now));
        assert!(is_up_to_date(&found(Some("/a"), 600), written(2), now));
        assert!(!is_up_to_date(&found(Some("/a"), 600), written(60), now));
        assert!(!is_up_to_date(&found(Some("/a"), 5), None, now));
        assert!(is_up_to_date(&found(None, 5), None, now));
        assert!(!is_up_to_date(&found(None, 600), None, now));
    }

    #[test]
    fn is_busy_only_after_a_recent_write() {
        let now = UNIX_EPOCH + Duration::from_secs(1_000);
        assert_eq!(status_of(now - Duration::from_secs(2), now), "busy");
        assert_eq!(status_of(now - Duration::from_secs(60), now), "idle");
        assert_eq!(status_of(now + Duration::from_secs(1), now), "busy");
    }
}
