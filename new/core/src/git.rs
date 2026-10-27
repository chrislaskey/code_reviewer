use std::process::Command;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::analysis::model::LineRange;

/// A single file changed in the git working tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangedFile {
    pub status: String,
    pub path: String,
}

/// Request to list changed files in a git repository.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListChangedFilesRequest {
    pub repo_path: String,
}

/// Response containing the list of changed files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListChangedFilesResponse {
    pub files: Vec<ChangedFile>,
}

#[derive(Debug, Error)]
pub enum GitError {
    #[error("failed to run git: {0}")]
    CommandFailed(String),

    #[error("git exited with status {code}: {stderr}")]
    NonZeroExit { code: i32, stderr: String },

    #[error("invalid revision: {0}")]
    InvalidRevision(String),

    #[error("unexpected git output: {0}")]
    UnexpectedOutput(String),
}

/// List files changed in the git working tree (staged + unstaged + untracked).
pub fn list_changed_files(
    request: ListChangedFilesRequest,
) -> Result<ListChangedFilesResponse, GitError> {
    let output = Command::new("git")
        .arg("status")
        .arg("--porcelain")
        .current_dir(&request.repo_path)
        .output()
        .map_err(|e| GitError::CommandFailed(e.to_string()))?;

    if !output.status.success() {
        let code = output.status.code().unwrap_or(-1);
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(GitError::NonZeroExit { code, stderr });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let files = stdout
        .lines()
        .filter(|line| !line.is_empty())
        .map(parse_porcelain_line)
        .collect();

    Ok(ListChangedFilesResponse { files })
}

/// Parse a single line from `git status --porcelain`.
/// Format: "XY path" where X=index status, Y=worktree status.
fn parse_porcelain_line(line: &str) -> ChangedFile {
    let status_code = &line[..2];
    let path = line[3..].to_string();

    let status = match status_code {
        " M" => "modified",
        " D" => "deleted",
        "M " => "modified",
        "A " => "added",
        "D " => "deleted",
        "R " | "RM" | "RD" => "renamed",
        "C " => "copied",
        "MM" => "modified (staged + unstaged)",
        "AM" => "added (modified since staging)",
        "AD" => "added (deleted since staging)",
        "??" => "untracked",
        "!!" => "ignored",
        other => other.trim(),
    }
    .to_string();

    ChangedFile { status, path }
}

/// Request to list the files a project consists of (tracked and untracked, not ignored).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListProjectFilePathsRequest {
    pub repo_path: String,
}

/// Paths relative to the repository path, sorted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListProjectFilePathsResponse {
    pub file_paths: Vec<String>,
}

/// Request to list the lines changed since `base_ref`, including uncommitted work.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListChangedLineRangesRequest {
    pub repo_path: String,
    /// Defaults to `HEAD`, which means all uncommitted changes.
    pub base_ref: Option<String>,
    /// Compare against this revision instead of the working tree.
    #[serde(default)]
    pub head_ref: Option<String>,
}

/// The changed lines of one file, as line ranges in the current file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangedFileLines {
    pub path: String,
    pub changed_ranges: Vec<LineRange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListChangedLineRangesResponse {
    pub files: Vec<ChangedFileLines>,
}

const DEFAULT_BASE_REF: &str = "HEAD";

fn run_git_bytes(repo_path: &str, arguments: &[&str]) -> Result<Vec<u8>, GitError> {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(repo_path)
        .output()
        .map_err(|e| GitError::CommandFailed(e.to_string()))?;

    if !output.status.success() {
        return Err(GitError::NonZeroExit {
            code: output.status.code().unwrap_or(-1),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }
    Ok(output.stdout)
}

fn run_git(repo_path: &str, arguments: &[&str]) -> Result<String, GitError> {
    run_git_bytes(repo_path, arguments).map(|bytes| String::from_utf8_lossy(&bytes).to_string())
}

/// List every file git tracks or could track (untracked files that are not ignored).
pub fn list_project_file_paths(
    request: ListProjectFilePathsRequest,
) -> Result<ListProjectFilePathsResponse, GitError> {
    let stdout = run_git(
        &request.repo_path,
        &[
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ],
    )?;

    let mut file_paths: Vec<String> = stdout
        .split('\0')
        .filter(|path| !path.is_empty())
        .map(str::to_string)
        .collect();
    file_paths.sort();
    file_paths.dedup();
    Ok(ListProjectFilePathsResponse { file_paths })
}

/// List the changed line ranges of every file between `base_ref` and
/// `head_ref`, or the working tree when there is no `head_ref`. Renames are
/// followed, so a moved file only lists the lines that changed. Untracked
/// files count as fully changed (working tree only). Deleted files are left
/// out because they have no lines in the new tree.
pub fn list_changed_line_ranges(
    request: ListChangedLineRangesRequest,
) -> Result<ListChangedLineRangesResponse, GitError> {
    let base_commit = get_commit_sha(
        &request.repo_path,
        request.base_ref.as_deref().unwrap_or(DEFAULT_BASE_REF),
    )?;
    let head_commit = request
        .head_ref
        .as_deref()
        .map(|head_ref| get_commit_sha(&request.repo_path, head_ref))
        .transpose()?;

    // Both are full SHAs from `get_commit_sha`, so they cannot be read as options.
    let mut arguments = vec![
        "-c",
        "core.quotePath=false",
        "diff",
        "-U0",
        "-M",
        "--no-ext-diff",
        "--no-color",
        "--relative",
        "--src-prefix=a/",
        "--dst-prefix=b/",
        base_commit.as_str(),
    ];
    if let Some(head_commit) = &head_commit {
        arguments.push(head_commit.as_str());
    }
    let mut files = parse_unified_diff_zero_context(&run_git(&request.repo_path, &arguments)?);

    if head_commit.is_none() {
        files.extend(list_untracked_files_as_changed(&request.repo_path)?);
    }

    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(ListChangedLineRangesResponse { files })
}

/// What happened to a path between the base and the head.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangedPathStatus {
    Added,
    Modified,
    Removed,
}

/// One path that differs between the base and the head, of any file type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedPath {
    pub path: String,
    /// The path in the base. Only set when the file was renamed or copied.
    pub previous_path: Option<String>,
    pub status: ChangedPathStatus,
}

/// List every path that differs between `base_commit` and `head_commit`, or
/// the working tree when there is no `head_commit`. Renames are followed.
/// Untracked files count as added (working tree only). Sorted by path.
pub fn list_changed_paths(
    repo_path: &str,
    base_commit: &str,
    head_commit: Option<&str>,
) -> Result<Vec<ChangedPath>, GitError> {
    // Both are full SHAs from `get_commit_sha`, so they cannot be read as options.
    let mut arguments = vec![
        "-c",
        "core.quotePath=false",
        "diff",
        "--name-status",
        "-z",
        "-M",
        "--no-ext-diff",
        "--no-color",
        "--relative",
        base_commit,
    ];
    if let Some(head_commit) = head_commit {
        arguments.push(head_commit);
    }
    let mut paths = parse_name_status(&run_git(repo_path, &arguments)?)?;

    if head_commit.is_none() {
        let untracked = run_git(
            repo_path,
            &["ls-files", "-z", "--others", "--exclude-standard"],
        )?;
        paths.extend(
            untracked
                .split('\0')
                .filter(|path| !path.is_empty())
                .map(|path| ChangedPath {
                    path: path.to_string(),
                    previous_path: None,
                    status: ChangedPathStatus::Added,
                }),
        );
    }

    paths.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(paths)
}

/// Parse `git diff --name-status -z`: `M\0path\0` and `R100\0old\0new\0`.
fn parse_name_status(output: &str) -> Result<Vec<ChangedPath>, GitError> {
    let mut fields = output.split('\0').filter(|field| !field.is_empty());
    let mut paths = Vec::new();
    while let Some(code) = fields.next() {
        let mut next_path = || {
            fields
                .next()
                .map(str::to_string)
                .ok_or_else(|| GitError::UnexpectedOutput(format!("no path after {code}")))
        };
        let changed = match code.chars().next() {
            Some('A') => ChangedPath {
                path: next_path()?,
                previous_path: None,
                status: ChangedPathStatus::Added,
            },
            Some('D') => ChangedPath {
                path: next_path()?,
                previous_path: None,
                status: ChangedPathStatus::Removed,
            },
            Some('R' | 'C') => {
                let previous_path = next_path()?;
                ChangedPath {
                    path: next_path()?,
                    previous_path: Some(previous_path),
                    status: ChangedPathStatus::Modified,
                }
            }
            Some('M' | 'T' | 'U' | 'X') => ChangedPath {
                path: next_path()?,
                previous_path: None,
                status: ChangedPathStatus::Modified,
            },
            _ => return Err(GitError::UnexpectedOutput(code.to_string())),
        };
        paths.push(changed);
    }
    Ok(paths)
}

/// Read one file as it was in a commit, as raw bytes.
pub fn get_file_bytes_at_commit(
    repo_path: &str,
    commit_sha: &str,
    file_path: &str,
) -> Result<Vec<u8>, GitError> {
    run_git_bytes(repo_path, &["show", &format!("{commit_sha}:{file_path}")])
}

fn list_untracked_files_as_changed(repo_path: &str) -> Result<Vec<ChangedFileLines>, GitError> {
    let untracked = run_git(
        repo_path,
        &["ls-files", "-z", "--others", "--exclude-standard"],
    )?;

    let mut files = Vec::new();
    for path in untracked.split('\0').filter(|path| !path.is_empty()) {
        let full_path = std::path::Path::new(repo_path).join(path);
        if let Ok(text) = std::fs::read_to_string(full_path) {
            let line_count = text.lines().count();
            if line_count > 0 {
                files.push(ChangedFileLines {
                    path: path.to_string(),
                    changed_ranges: vec![LineRange {
                        start_line: 1,
                        end_line: line_count,
                    }],
                });
            }
        }
    }
    Ok(files)
}

/// Resolve a branch, tag, SHA or expression like `HEAD~2` to a full commit SHA.
pub fn get_commit_sha(repo_path: &str, revision: &str) -> Result<String, GitError> {
    if revision.is_empty() || revision.starts_with('-') || revision.contains('\0') {
        return Err(GitError::InvalidRevision(revision.to_string()));
    }

    let object = format!("{revision}^{{commit}}");
    let stdout = run_git(repo_path, &["rev-parse", "--verify", "--quiet", &object])
        .map_err(|_| GitError::InvalidRevision(revision.to_string()))?;
    Ok(stdout.trim().to_string())
}

/// The best common ancestor of two revisions, as a full SHA. Reviewing from
/// here shows only what one side changed, as a pull request does.
pub fn get_merge_base_commit_sha(
    repo_path: &str,
    first_revision: &str,
    second_revision: &str,
) -> Result<String, GitError> {
    let first = get_commit_sha(repo_path, first_revision)?;
    let second = get_commit_sha(repo_path, second_revision)?;
    let stdout = run_git(repo_path, &["merge-base", &first, &second])?;
    Ok(stdout.trim().to_string())
}

/// Request for the branches and latest commits to choose a comparison from.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListBranchesAndRecentCommitsRequest {
    pub repo_path: String,
    /// Defaults to 30.
    #[serde(default)]
    pub recent_commit_count: Option<usize>,
}

/// One commit, as shown in a list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitSummary {
    pub sha: String,
    pub short_sha: String,
    pub subject: String,
    /// `YYYY-MM-DD`.
    pub committed_on: String,
}

/// Branches and recent commits of a repository.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListBranchesAndRecentCommitsResponse {
    /// `None` when `HEAD` is detached.
    pub current_branch: Option<String>,
    /// The branch pull requests usually target: `origin/HEAD`, else `main` or `master`.
    pub default_branch: Option<String>,
    /// Local branches first, then remote branches that have no local copy.
    pub branches: Vec<String>,
    /// Newest first, reachable from `HEAD`. Empty in a repository without commits.
    pub recent_commits: Vec<CommitSummary>,
}

const DEFAULT_RECENT_COMMIT_COUNT: usize = 30;

/// List the branches and the latest commits, for choosing what to review.
pub fn list_branches_and_recent_commits(
    request: ListBranchesAndRecentCommitsRequest,
) -> Result<ListBranchesAndRecentCommitsResponse, GitError> {
    let repo = request.repo_path.as_str();
    let local_branches = list_ref_names(repo, "refs/heads/")?;
    let remote_branches: Vec<String> = list_ref_names(repo, "refs/remotes/")?
        .into_iter()
        .filter(|name| !name.ends_with("/HEAD"))
        .filter(|name| {
            let without_remote = name.split_once('/').map_or(name.as_str(), |(_, rest)| rest);
            !local_branches.iter().any(|local| local == without_remote)
        })
        .collect();

    let current_branch = Some(
        run_git(repo, &["branch", "--show-current"])?
            .trim()
            .to_string(),
    )
    .filter(|name| !name.is_empty());
    let default_branch = find_default_branch(repo, &local_branches, &remote_branches);

    let count = request
        .recent_commit_count
        .unwrap_or(DEFAULT_RECENT_COMMIT_COUNT)
        .to_string();
    let recent_commits = run_git(
        repo,
        &[
            "log",
            "-n",
            &count,
            "--date=short",
            "--format=%H%x1f%h%x1f%cd%x1f%s",
        ],
    )
    .map(|stdout| stdout.lines().filter_map(parse_commit_summary).collect())
    .unwrap_or_default();

    let mut branches = local_branches;
    branches.extend(remote_branches);
    Ok(ListBranchesAndRecentCommitsResponse {
        current_branch,
        default_branch,
        branches,
        recent_commits,
    })
}

fn list_ref_names(repo_path: &str, prefix: &str) -> Result<Vec<String>, GitError> {
    let stdout = run_git(
        repo_path,
        &[
            "for-each-ref",
            "--format=%(refname)",
            "--sort=refname",
            prefix,
        ],
    )?;
    Ok(stdout
        .lines()
        .filter_map(|name| {
            name.strip_prefix("refs/heads/")
                .or_else(|| name.strip_prefix("refs/remotes/"))
        })
        .map(str::to_string)
        .collect())
}

fn find_default_branch(
    repo_path: &str,
    local_branches: &[String],
    remote_branches: &[String],
) -> Option<String> {
    let remote_default = run_git(
        repo_path,
        &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
    )
    .ok()
    .map(|stdout| stdout.trim().to_string());

    if let Some(remote_default) = remote_default {
        let local_name = remote_default
            .split_once('/')
            .map_or(remote_default.as_str(), |(_, rest)| rest);
        if local_branches.iter().any(|local| local == local_name) {
            return Some(local_name.to_string());
        }
        if remote_branches.contains(&remote_default) {
            return Some(remote_default);
        }
    }
    ["main", "master"]
        .into_iter()
        .find(|name| local_branches.iter().any(|local| local == name))
        .map(str::to_string)
}

fn parse_commit_summary(line: &str) -> Option<CommitSummary> {
    let mut parts = line.splitn(4, '\x1f');
    Some(CommitSummary {
        sha: parts.next()?.to_string(),
        short_sha: parts.next()?.to_string(),
        committed_on: parts.next()?.to_string(),
        subject: parts.next()?.to_string(),
    })
}

/// The first parent of a commit, as a full SHA.
pub fn get_parent_commit_sha(repo_path: &str, commit_sha: &str) -> Result<String, GitError> {
    get_commit_sha(repo_path, &format!("{commit_sha}^"))
}

/// List every file path in the tree of a commit.
pub fn list_file_paths_at_commit(
    repo_path: &str,
    commit_sha: &str,
) -> Result<Vec<String>, GitError> {
    let stdout = run_git(
        repo_path,
        &["ls-tree", "-r", "-z", "--name-only", commit_sha],
    )?;
    Ok(stdout
        .split('\0')
        .filter(|path| !path.is_empty())
        .map(str::to_string)
        .collect())
}

/// Read one file as it was in a commit.
pub fn get_file_text_at_commit(
    repo_path: &str,
    commit_sha: &str,
    file_path: &str,
) -> Result<String, GitError> {
    run_git(repo_path, &["show", &format!("{commit_sha}:{file_path}")])
}

/// Read many files as they were in a commit with a single git process.
/// Files that are missing or are not UTF-8 text are left out.
pub fn list_file_texts_at_commit(
    repo_path: &str,
    commit_sha: &str,
    file_paths: &[String],
) -> Result<Vec<(String, String)>, GitError> {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::process::Stdio;

    let readable_paths: Vec<&String> = file_paths
        .iter()
        .filter(|path| !path.contains('\n'))
        .collect();

    let mut child = Command::new("git")
        .args(["cat-file", "--batch"])
        .current_dir(repo_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| GitError::CommandFailed(e.to_string()))?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| GitError::CommandFailed("no stdin for git cat-file".to_string()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| GitError::CommandFailed("no stdout for git cat-file".to_string()))?;

    // Write requests on another thread so a full pipe cannot block the reader.
    let requests: Vec<String> = readable_paths
        .iter()
        .map(|path| format!("{commit_sha}:{path}\n"))
        .collect();
    let writer = std::thread::spawn(move || {
        for request in requests {
            if stdin.write_all(request.as_bytes()).is_err() {
                break;
            }
        }
    });

    let mut reader = BufReader::new(stdout);
    let mut texts = Vec::new();
    let mut read_result: Result<(), GitError> = Ok(());
    for path in &readable_paths {
        let mut header = String::new();
        match reader.read_line(&mut header) {
            Ok(0) => {
                read_result = Err(GitError::UnexpectedOutput(
                    "git cat-file ended early".to_string(),
                ));
                break;
            }
            Ok(_) => {}
            Err(e) => {
                read_result = Err(GitError::CommandFailed(e.to_string()));
                break;
            }
        }

        // "<oid> <type> <size>" for a found object, "<spec> missing" otherwise.
        let size = match header
            .trim_end()
            .rsplit(' ')
            .next()
            .map(str::parse::<usize>)
        {
            Some(Ok(size)) if !header.trim_end().ends_with("missing") => size,
            _ => continue,
        };
        let mut content = vec![0u8; size + 1];
        if let Err(e) = reader.read_exact(&mut content) {
            read_result = Err(GitError::CommandFailed(e.to_string()));
            break;
        }
        content.truncate(size);
        if let Ok(text) = String::from_utf8(content) {
            texts.push(((*path).clone(), text));
        }
    }

    drop(reader);
    let _ = writer.join();
    let _ = child.wait();
    read_result.map(|()| texts)
}

/// Parse `git diff -U0` output into new-side line ranges per file.
///
/// A pure deletion has no new lines, so it is recorded as the two lines
/// around the removal point.
fn parse_unified_diff_zero_context(diff: &str) -> Vec<ChangedFileLines> {
    let mut files: Vec<ChangedFileLines> = Vec::new();
    let mut current: Option<ChangedFileLines> = None;

    for line in diff.lines() {
        if let Some(path) = line.strip_prefix("+++ ") {
            if let Some(finished) = current.take() {
                files.push(finished);
            }
            current = path.strip_prefix("b/").map(|path| ChangedFileLines {
                path: path.to_string(),
                changed_ranges: Vec::new(),
            });
        } else if line.starts_with("@@") {
            if let (Some(file), Some(range)) = (current.as_mut(), parse_hunk_new_range(line)) {
                file.changed_ranges.push(range);
            }
        }
    }
    files.extend(current);
    files.retain(|file| !file.changed_ranges.is_empty());
    files
}

/// Parse the `+start,count` part of `@@ -a,b +start,count @@`.
fn parse_hunk_new_range(hunk_header: &str) -> Option<LineRange> {
    let new_side = hunk_header
        .split_whitespace()
        .find(|part| part.starts_with('+'))?;
    let (start, count) = match new_side[1..].split_once(',') {
        Some((start, count)) => (start.parse::<usize>().ok()?, count.parse::<usize>().ok()?),
        None => (new_side[1..].parse::<usize>().ok()?, 1),
    };

    Some(if count == 0 {
        LineRange {
            start_line: start.max(1),
            end_line: start + 1,
        }
    } else {
        LineRange {
            start_line: start,
            end_line: start + count - 1,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_name_status_reads_every_kind_of_change() {
        let output = "A\0new.md\0M\0lib/a.ex\0D\0old.txt\0R087\0from.md\0to.md\0";

        let paths = parse_name_status(output).unwrap();

        let summary: Vec<(&str, Option<&str>, ChangedPathStatus)> = paths
            .iter()
            .map(|p| (p.path.as_str(), p.previous_path.as_deref(), p.status))
            .collect();
        assert_eq!(
            summary,
            [
                ("new.md", None, ChangedPathStatus::Added),
                ("lib/a.ex", None, ChangedPathStatus::Modified),
                ("old.txt", None, ChangedPathStatus::Removed),
                ("to.md", Some("from.md"), ChangedPathStatus::Modified),
            ]
        );
    }

    #[test]
    fn test_parse_name_status_rejects_a_missing_path() {
        assert!(parse_name_status("M\0").is_err());
        assert!(parse_name_status("Z\0x\0").is_err());
    }

    #[test]
    fn test_parse_porcelain_modified() {
        let file = parse_porcelain_line(" M src/main.rs");
        assert_eq!(file.status, "modified");
        assert_eq!(file.path, "src/main.rs");
    }

    #[test]
    fn test_parse_porcelain_untracked() {
        let file = parse_porcelain_line("?? new_file.txt");
        assert_eq!(file.status, "untracked");
        assert_eq!(file.path, "new_file.txt");
    }

    #[test]
    fn test_parse_porcelain_added() {
        let file = parse_porcelain_line("A  added.rs");
        assert_eq!(file.status, "added");
        assert_eq!(file.path, "added.rs");
    }

    #[test]
    fn test_list_changed_files_on_this_repo() {
        let repo_path = env!("CARGO_MANIFEST_DIR")
            .strip_suffix("/core")
            .unwrap_or(env!("CARGO_MANIFEST_DIR"));

        let result = list_changed_files(ListChangedFilesRequest {
            repo_path: repo_path.to_string(),
        });
        assert!(result.is_ok(), "should be able to read git status");
    }

    #[test]
    fn test_parse_unified_diff_zero_context_reads_new_side_ranges() {
        let diff = "diff --git a/lib/a.ex b/lib/a.ex\n--- a/lib/a.ex\n+++ b/lib/a.ex\n@@ -3,0 +4,2 @@ x\n+a\n+b\n@@ -10 +12 @@\n-c\n+d\n@@ -20,2 +21,0 @@\n-e\n-f\n";
        let files = parse_unified_diff_zero_context(diff);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "lib/a.ex");
        assert_eq!(
            files[0].changed_ranges,
            vec![
                LineRange {
                    start_line: 4,
                    end_line: 5
                },
                LineRange {
                    start_line: 12,
                    end_line: 12
                },
                LineRange {
                    start_line: 21,
                    end_line: 22
                },
            ]
        );
    }

    #[test]
    fn test_parse_unified_diff_skips_deleted_files() {
        let diff =
            "diff --git a/old.ex b/old.ex\n--- a/old.ex\n+++ /dev/null\n@@ -1,3 +0,0 @@\n-a\n";
        assert!(parse_unified_diff_zero_context(diff).is_empty());
    }
}
