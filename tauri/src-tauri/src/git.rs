//! Reads the git status of a directory: the repository's name, branch, how
//! far it is ahead of and behind its upstream, and how many files are
//! staged, unstaged, untracked and conflicted.

use std::path::Path;
use std::time::Duration;

use serde::Serialize;
use tokio::process::Command;

/// How long `git status` may take, so a huge repository can't hold up the
/// status page.
const GIT_STATUS_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct GitStatus {
    /// The main checkout's name, shared by its worktrees, e.g. `code_reviewer`.
    pub repository_name: Option<String>,
    /// `None` when HEAD is detached.
    pub branch: Option<String>,
    /// The first 7 characters of HEAD's commit, `None` before the first
    /// commit.
    pub commit: Option<String>,
    pub upstream: Option<String>,
    /// Commits on the branch that aren't on its upstream.
    pub ahead: u32,
    /// Commits on the upstream that aren't on the branch.
    pub behind: u32,
    /// Files with changes in the index.
    pub staged: u32,
    /// Tracked files with changes that aren't in the index.
    pub unstaged: u32,
    pub untracked: u32,
    /// Files with merge conflicts.
    pub conflicted: u32,
    /// Whether any file is staged, unstaged, untracked or conflicted.
    pub dirty: bool,
    /// The kind of change most in need of attention, `None` when clean.
    pub most_important_change: Option<GitChange>,
}

/// A kind of uncommitted change, most in need of attention first.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GitChange {
    Conflicted,
    Unstaged,
    Untracked,
    Staged,
}

/// The git status of `directory`, or `None` when it isn't in a git
/// repository, git isn't installed or it took too long.
pub async fn get_status(directory: &str) -> Option<GitStatus> {
    // `--no-optional-locks` keeps a background `git status` from taking the
    // index lock while the user runs git commands.
    let output = Command::new("git")
        .args(["--no-optional-locks", "-C", directory])
        .args([
            "status",
            "--porcelain=v2",
            "--branch",
            "--untracked-files=normal",
        ])
        .kill_on_drop(true)
        .output();
    let output = tokio::time::timeout(GIT_STATUS_TIMEOUT, output)
        .await
        .ok()?
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let mut status = parse_status(&String::from_utf8_lossy(&output.stdout));
    status.repository_name = get_repository_name(directory).await;
    Some(status)
}

async fn get_repository_name(directory: &str) -> Option<String> {
    let output = Command::new("git")
        .args(["-C", directory])
        .args(["rev-parse", "--path-format=absolute", "--git-common-dir"])
        .kill_on_drop(true)
        .output();
    let output = tokio::time::timeout(GIT_STATUS_TIMEOUT, output)
        .await
        .ok()?
        .ok()?;
    if !output.status.success() {
        return None;
    }
    repository_name_of(String::from_utf8_lossy(&output.stdout).trim())
}

/// `/code/app/.git` is `app`'s, and a bare `/code/app.git` is `app`.
fn repository_name_of(common_dir: &str) -> Option<String> {
    let common_dir = Path::new(common_dir);
    let repository = if common_dir.file_name()? == ".git" {
        common_dir.parent()?
    } else {
        common_dir
    };
    let name = repository.file_name()?.to_string_lossy();
    Some(name.strip_suffix(".git").unwrap_or(&name).to_string())
}

/// Parses `git status --porcelain=v2 --branch`, see `git help status`.
fn parse_status(output: &str) -> GitStatus {
    let mut status = GitStatus::default();
    for line in output.lines() {
        if let Some(header) = line.strip_prefix("# ") {
            parse_branch_header(header, &mut status);
            continue;
        }
        match line.split(' ').next() {
            // Changed (1) and renamed or copied (2) entries start with XY:
            // the index's change, then the work tree's, `.` for none.
            Some("1" | "2") => {
                let mut xy = line[2..].chars();
                if xy.next().is_some_and(|x| x != '.') {
                    status.staged += 1;
                }
                if xy.next().is_some_and(|y| y != '.') {
                    status.unstaged += 1;
                }
            }
            Some("u") => status.conflicted += 1,
            Some("?") => status.untracked += 1,
            _ => {}
        }
    }
    status.dirty = status.staged + status.unstaged + status.untracked + status.conflicted > 0;
    status.most_important_change = most_important_change(&status);
    status
}

fn most_important_change(status: &GitStatus) -> Option<GitChange> {
    [
        (status.conflicted, GitChange::Conflicted),
        (status.unstaged, GitChange::Unstaged),
        (status.untracked, GitChange::Untracked),
        (status.staged, GitChange::Staged),
    ]
    .into_iter()
    .find(|(count, _)| *count > 0)
    .map(|(_, change)| change)
}

fn parse_branch_header(header: &str, status: &mut GitStatus) {
    let Some((key, value)) = header.split_once(' ') else {
        return;
    };
    match key {
        "branch.oid" if value != "(initial)" => {
            status.commit = Some(value.chars().take(7).collect());
        }
        "branch.head" if value != "(detached)" => status.branch = Some(value.to_string()),
        "branch.upstream" => status.upstream = Some(value.to_string()),
        "branch.ab" => {
            for count in value.split(' ') {
                if let Some(ahead) = count.strip_prefix('+') {
                    status.ahead = ahead.parse().unwrap_or(0);
                } else if let Some(behind) = count.strip_prefix('-') {
                    status.behind = behind.parse().unwrap_or(0);
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_porcelain_v2() {
        let output = "\
# branch.oid 1234567890abcdef
# branch.head main
# branch.upstream origin/main
# branch.ab +2 -1
1 M. N... 100644 100644 100644 abc abc staged.rs
1 .M N... 100644 100644 100644 abc abc unstaged.rs
1 MM N... 100644 100644 100644 abc abc both.rs
2 R. N... 100644 100644 100644 abc abc R100 new.rs\told.rs
u UU N... 100644 100644 100644 100644 abc abc abc conflict.rs
? untracked.rs
? another untracked.rs
";
        assert_eq!(
            parse_status(output),
            GitStatus {
                repository_name: None,
                branch: Some("main".into()),
                commit: Some("1234567".into()),
                upstream: Some("origin/main".into()),
                ahead: 2,
                behind: 1,
                staged: 3,
                unstaged: 2,
                untracked: 2,
                conflicted: 1,
                dirty: true,
                most_important_change: Some(GitChange::Conflicted),
            }
        );
    }

    #[test]
    fn ranks_unstaged_over_untracked_over_staged() {
        let change = |output: &str| parse_status(output).most_important_change;
        let staged = "1 M. N... 100644 100644 100644 abc abc staged.rs\n";
        let untracked = "? untracked.rs\n";
        let unstaged = "1 .M N... 100644 100644 100644 abc abc unstaged.rs\n";
        assert_eq!(change(""), None);
        assert_eq!(change(staged), Some(GitChange::Staged));
        assert_eq!(
            change(&format!("{staged}{untracked}")),
            Some(GitChange::Untracked)
        );
        assert_eq!(
            change(&format!("{staged}{untracked}{unstaged}")),
            Some(GitChange::Unstaged)
        );
    }

    #[test]
    fn parses_detached_and_initial_heads() {
        let detached = parse_status("# branch.oid 1234567890\n# branch.head (detached)\n");
        assert_eq!(detached.branch, None);
        assert_eq!(detached.commit.as_deref(), Some("1234567"));
        assert!(!detached.dirty);

        let initial = parse_status("# branch.oid (initial)\n# branch.head main\n");
        assert_eq!(initial.commit, None);
    }

    #[test]
    fn names_the_repository_from_its_common_dir() {
        assert_eq!(
            repository_name_of("/code/code_reviewer/.git").as_deref(),
            Some("code_reviewer")
        );
        assert_eq!(repository_name_of("/code/app.git").as_deref(), Some("app"));
        assert_eq!(repository_name_of("/").as_deref(), None);
    }

    #[tokio::test]
    async fn reads_a_real_repository() {
        let directory =
            std::env::temp_dir().join(format!("code-reviewer-git-{}", std::process::id()));
        std::fs::create_dir_all(&directory).expect("directory is created");
        let path = directory.display().to_string();
        let git = |args: &[&str]| {
            std::process::Command::new("git")
                .args(["-C", &path])
                .args(args)
                .output()
                .map(|output| output.status.success())
                .unwrap_or(false)
        };
        if !git(&["init", "-q", "-b", "test-branch"]) {
            std::fs::remove_dir_all(&directory).ok();
            return;
        }
        std::fs::write(directory.join("staged.txt"), "one").expect("file is written");
        git(&["add", "staged.txt"]);
        std::fs::write(directory.join("staged.txt"), "two").expect("file is written");
        std::fs::write(directory.join("untracked.txt"), "new").expect("file is written");

        let status = get_status(&path).await;
        let outside = get_status("/").await;
        std::fs::remove_dir_all(&directory).expect("directory is removed");

        let status = status.expect("it's a repository");
        assert_eq!(status.branch.as_deref(), Some("test-branch"));
        assert_eq!(
            status.repository_name,
            directory
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
        );
        assert_eq!(
            (status.staged, status.unstaged, status.untracked),
            (1, 1, 1)
        );
        assert!(status.dirty);
        assert_eq!(status.most_important_change, Some(GitChange::Unstaged));
        assert_eq!(outside, None);
    }
}
