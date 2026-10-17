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

fn run_git(repo_path: &str, arguments: &[&str]) -> Result<String, GitError> {
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
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
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

/// List the changed line ranges of every file since `base_ref`. Untracked
/// files count as fully changed. Deleted files are left out because they
/// have no lines in the current tree.
pub fn list_changed_line_ranges(
    request: ListChangedLineRangesRequest,
) -> Result<ListChangedLineRangesResponse, GitError> {
    let base_ref = request.base_ref.as_deref().unwrap_or(DEFAULT_BASE_REF);
    let diff = run_git(
        &request.repo_path,
        &[
            "-c",
            "core.quotePath=false",
            "diff",
            "-U0",
            "--no-renames",
            "--no-ext-diff",
            "--no-color",
            "--relative",
            "--src-prefix=a/",
            "--dst-prefix=b/",
            base_ref,
        ],
    )?;
    let mut files = parse_unified_diff_zero_context(&diff);

    let untracked = run_git(
        &request.repo_path,
        &["ls-files", "-z", "--others", "--exclude-standard"],
    )?;
    for path in untracked.split('\0').filter(|path| !path.is_empty()) {
        let full_path = std::path::Path::new(&request.repo_path).join(path);
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

    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(ListChangedLineRangesResponse { files })
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
