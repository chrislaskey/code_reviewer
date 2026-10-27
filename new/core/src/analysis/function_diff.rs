use serde::{Deserialize, Serialize};
use similar::{ChangeTag, TextDiff};

use super::error::AnalysisError;
use super::function_source::{get_function_source, FunctionSource};
use super::model::FunctionId;
use super::project_index::ProjectIndex;

/// How a line of a function differs between the base and the head.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffLineKind {
    Context,
    Added,
    Removed,
}

/// One line of a function with its change. Context lines have both numbers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    /// The line in the base. `None` for added lines.
    pub old_number: Option<usize>,
    /// The line in the head. `None` for removed lines.
    pub new_number: Option<usize>,
    pub text: String,
}

/// Which function to show, by its name in the base and in the head.
///
/// - Added: only `new_function`.
/// - Removed: only `old_function`.
/// - Modified: both, with the same id.
/// - Renamed: both, with different ids.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionDiffTarget {
    pub old_function: Option<FunctionId>,
    pub new_function: Option<FunctionId>,
}

/// The whole function, every line marked as unchanged, added or removed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionDiff {
    pub old_function: Option<FunctionId>,
    pub new_function: Option<FunctionId>,
    /// The file of the head version, or else of the base version.
    pub file_path: String,
    pub lines: Vec<DiffLine>,
}

/// Line by line difference between two versions of a function. Either side may be missing.
pub fn diff_function_sources(
    old: Option<&FunctionSource>,
    new: Option<&FunctionSource>,
) -> Vec<DiffLine> {
    let old_lines = old.map_or(&[][..], |source| source.lines.as_slice());
    let new_lines = new.map_or(&[][..], |source| source.lines.as_slice());
    let old_texts: Vec<&str> = old_lines.iter().map(|line| line.text.as_str()).collect();
    let new_texts: Vec<&str> = new_lines.iter().map(|line| line.text.as_str()).collect();

    TextDiff::from_slices(&old_texts, &new_texts)
        .iter_all_changes()
        .map(|change| {
            let old_number = change.old_index().map(|index| old_lines[index].number);
            let new_number = change.new_index().map(|index| new_lines[index].number);
            DiffLine {
                kind: match change.tag() {
                    ChangeTag::Equal => DiffLineKind::Context,
                    ChangeTag::Delete => DiffLineKind::Removed,
                    ChangeTag::Insert => DiffLineKind::Added,
                },
                old_number,
                new_number,
                text: change.value().to_string(),
            }
        })
        .collect()
}

/// Read the function from each side's index and mark which of its lines changed.
pub fn get_function_diff(
    base_index: &ProjectIndex,
    head_index: &ProjectIndex,
    target: &FunctionDiffTarget,
) -> Result<FunctionDiff, AnalysisError> {
    let old = target
        .old_function
        .as_ref()
        .map(|function| get_function_source(base_index, function))
        .transpose()?;
    let new = target
        .new_function
        .as_ref()
        .map(|function| get_function_source(head_index, function))
        .transpose()?;
    let file_path = new
        .as_ref()
        .or(old.as_ref())
        .map(|source| source.file_path.clone())
        .ok_or(AnalysisError::EmptyFunctionDiffTarget)?;

    Ok(FunctionDiff {
        old_function: target.old_function.clone(),
        new_function: target.new_function.clone(),
        file_path,
        lines: diff_function_sources(old.as_ref(), new.as_ref()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::function_source::SourceLine;

    fn source(first_line: usize, texts: &[&str]) -> FunctionSource {
        FunctionSource {
            function: FunctionId::new("Shop.Orders", "total", 1),
            file_path: "lib/shop/orders.ex".to_string(),
            start_line: first_line,
            end_line: first_line + texts.len() - 1,
            lines: texts
                .iter()
                .enumerate()
                .map(|(offset, text)| SourceLine {
                    number: first_line + offset,
                    text: text.to_string(),
                })
                .collect(),
        }
    }

    fn marks(lines: &[DiffLine]) -> Vec<(char, Option<usize>, Option<usize>)> {
        lines
            .iter()
            .map(|line| {
                let mark = match line.kind {
                    DiffLineKind::Context => ' ',
                    DiffLineKind::Added => '+',
                    DiffLineKind::Removed => '-',
                };
                (mark, line.old_number, line.new_number)
            })
            .collect()
    }

    #[test]
    fn test_a_changed_line_is_removed_then_added_with_both_numbers_on_the_rest() {
        let old = source(10, &["def total(x) do", "  x + 1", "end"]);
        let new = source(12, &["def total(x) do", "  x + 2", "  x", "end"]);

        let lines = diff_function_sources(Some(&old), Some(&new));

        assert_eq!(
            marks(&lines),
            [
                (' ', Some(10), Some(12)),
                ('-', Some(11), None),
                ('+', None, Some(13)),
                ('+', None, Some(14)),
                (' ', Some(12), Some(15)),
            ]
        );
        assert_eq!(lines[1].text, "  x + 1");
        assert_eq!(lines[2].text, "  x + 2");
    }

    #[test]
    fn test_an_added_function_is_all_added_lines() {
        let new = source(1, &["def a, do: 1", "def b, do: 2"]);

        let lines = diff_function_sources(None, Some(&new));

        assert_eq!(marks(&lines), [('+', None, Some(1)), ('+', None, Some(2))]);
    }

    #[test]
    fn test_a_removed_function_is_all_removed_lines() {
        let old = source(5, &["def a, do: 1"]);

        let lines = diff_function_sources(Some(&old), None);

        assert_eq!(marks(&lines), [('-', Some(5), None)]);
    }

    #[test]
    fn test_identical_sources_are_all_context() {
        let old = source(1, &["a", "b"]);
        let new = source(4, &["a", "b"]);

        let lines = diff_function_sources(Some(&old), Some(&new));

        assert!(lines.iter().all(|line| line.kind == DiffLineKind::Context));
    }
}
