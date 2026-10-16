use std::process::Command;

use serde::{Deserialize, Serialize};
use thiserror::Error;

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
}
