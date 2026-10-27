use std::path::Path;

use serde::{Deserialize, Serialize};
use similar::{ChangeTag, TextDiff};

use super::error::AnalysisError;
use super::function_diff::{DiffLine, DiffLineKind};
use super::markdown_html::{is_markdown_path, render_new_version_hunks};
use crate::git::get_file_bytes_at_commit;

/// Lines of unchanged text kept around each change.
const CONTEXT_LINE_COUNT: usize = 3;

/// A file shows at most this many lines. A lock file should not freeze the page.
pub const MAX_FILE_DIFF_LINES: usize = 2000;

/// Which file to show, by its path in the base and in the head.
///
/// - Added: only `new_path`.
/// - Removed: only `old_path`.
/// - Modified: both, with the same path.
/// - Renamed: both, with different paths.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileDiffTarget {
    pub old_path: Option<String>,
    pub new_path: Option<String>,
}

/// The changed lines of a file with a few lines of context around them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileDiff {
    pub old_path: Option<String>,
    pub new_path: Option<String>,
    /// Empty for a binary file.
    pub lines: Vec<DiffLine>,
    pub is_binary: bool,
    /// Lines left out after `MAX_FILE_DIFF_LINES`.
    pub omitted_line_count: usize,
    /// True for a Markdown file, which also has `html_hunks`.
    pub is_markdown: bool,
    /// The new version of each stretch of changed lines as HTML, with the blocks that
    /// hold added lines marked by a `data-added` attribute. Empty unless Markdown.
    pub html_hunks: Vec<String>,
}

/// Line by line difference of two texts, keeping only the changes and their context.
pub fn diff_texts(old: &str, new: &str) -> Vec<DiffLine> {
    let diff = TextDiff::from_lines(old, new);
    let mut lines = Vec::new();
    for group in diff.grouped_ops(CONTEXT_LINE_COUNT) {
        for change in group.iter().flat_map(|op| diff.iter_changes(op)) {
            lines.push(DiffLine {
                kind: match change.tag() {
                    ChangeTag::Equal => DiffLineKind::Context,
                    ChangeTag::Delete => DiffLineKind::Removed,
                    ChangeTag::Insert => DiffLineKind::Added,
                },
                old_number: change.old_index().map(|index| index + 1),
                new_number: change.new_index().map(|index| index + 1),
                text: change.value().trim_end_matches(['\n', '\r']).to_string(),
            });
        }
    }
    lines
}

/// The text of a file, or `None` when it is binary.
fn read_text(
    repo_path: &str,
    commit: Option<&str>,
    path: &str,
) -> Result<Option<String>, AnalysisError> {
    let bytes = match commit {
        Some(commit) => get_file_bytes_at_commit(repo_path, commit, path)?,
        None => std::fs::read(Path::new(repo_path).join(path)).map_err(|error| {
            AnalysisError::ReadFileFailed {
                path: path.to_string(),
                message: error.to_string(),
            }
        })?,
    };
    if bytes.contains(&0) {
        return Ok(None);
    }
    Ok(String::from_utf8(bytes).ok())
}

/// Compare a file in the base with the same file in the head. `head_commit`
/// `None` reads the head from the working tree.
pub fn get_file_diff(
    repo_path: &str,
    base_commit: &str,
    head_commit: Option<&str>,
    target: &FileDiffTarget,
) -> Result<FileDiff, AnalysisError> {
    if target.old_path.is_none() && target.new_path.is_none() {
        return Err(AnalysisError::EmptyFileDiffTarget);
    }
    let old = target
        .old_path
        .as_deref()
        .map(|path| read_text(repo_path, Some(base_commit), path))
        .transpose()?;
    let new = target
        .new_path
        .as_deref()
        .map(|path| read_text(repo_path, head_commit, path))
        .transpose()?;

    let is_binary = matches!(old, Some(None)) || matches!(new, Some(None));
    let mut lines = if is_binary {
        Vec::new()
    } else {
        diff_texts(
            old.flatten().as_deref().unwrap_or_default(),
            new.flatten().as_deref().unwrap_or_default(),
        )
    };
    let omitted_line_count = lines.len().saturating_sub(MAX_FILE_DIFF_LINES);
    lines.truncate(MAX_FILE_DIFF_LINES);

    let is_markdown = target
        .new_path
        .as_deref()
        .or(target.old_path.as_deref())
        .is_some_and(is_markdown_path);
    let html_hunks = if is_markdown {
        render_new_version_hunks(&lines)
    } else {
        Vec::new()
    };

    Ok(FileDiff {
        old_path: target.old_path.clone(),
        new_path: target.new_path.clone(),
        lines,
        is_binary,
        omitted_line_count,
        is_markdown,
        html_hunks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(lines: &[DiffLine]) -> Vec<DiffLineKind> {
        lines.iter().map(|line| line.kind).collect()
    }

    #[test]
    fn test_a_new_text_is_all_added_lines_with_new_numbers() {
        let lines = diff_texts("", "one\ntwo\n");

        assert_eq!(kinds(&lines), [DiffLineKind::Added, DiffLineKind::Added]);
        assert_eq!(lines[1].new_number, Some(2));
        assert_eq!(lines[1].old_number, None);
        assert_eq!(lines[1].text, "two");
    }

    #[test]
    fn test_a_removed_text_is_all_removed_lines() {
        let lines = diff_texts("one\ntwo\n", "");

        assert_eq!(
            kinds(&lines),
            [DiffLineKind::Removed, DiffLineKind::Removed]
        );
        assert_eq!(lines[0].old_number, Some(1));
    }

    #[test]
    fn test_a_change_keeps_only_a_few_lines_of_context() {
        let old: String = (1..=20).map(|n| format!("line {n}\n")).collect();
        let new = old.replace("line 10\n", "changed\n");

        let lines = diff_texts(&old, &new);

        // 3 before, the removed and added line, 3 after.
        assert_eq!(lines.len(), 8);
        assert_eq!(lines[0].text, "line 7");
        assert_eq!(lines[7].text, "line 13");
        assert_eq!(lines[3].kind, DiffLineKind::Removed);
        assert_eq!(lines[4].kind, DiffLineKind::Added);
    }

    #[test]
    fn test_identical_texts_have_no_lines() {
        assert!(diff_texts("same\n", "same\n").is_empty());
    }
}
