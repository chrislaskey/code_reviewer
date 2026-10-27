use serde::{Deserialize, Serialize};

use super::model::FunctionId;
use super::project_index::ProjectIndex;
use crate::git::ChangedFileLines;

/// A function that contains at least one changed line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangedFunction {
    pub function: FunctionId,
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    /// The changed lines that fall inside the function, ascending.
    pub changed_lines: Vec<usize>,
}

/// Match changed line ranges to the functions they touch.
///
/// Only functions in the current tree are found, so functions that were
/// deleted have no entry.
pub fn list_changed_functions(
    index: &ProjectIndex,
    changed_files: &[ChangedFileLines],
) -> Vec<ChangedFunction> {
    let mut changed_functions = Vec::new();
    for file in changed_files {
        for function in index.list_functions_in_file(&file.path) {
            let function_range = function.line_range();
            let mut changed_lines: Vec<usize> = file
                .changed_ranges
                .iter()
                .filter(|range| range.overlaps(&function_range))
                .flat_map(|range| {
                    range.start_line.max(function_range.start_line)
                        ..=range.end_line.min(function_range.end_line)
                })
                .collect();
            changed_lines.sort_unstable();
            changed_lines.dedup();

            if !changed_lines.is_empty() {
                changed_functions.push(ChangedFunction {
                    function: function.id.clone(),
                    file_path: function.file_path.clone(),
                    start_line: function.start_line,
                    end_line: function.end_line,
                    changed_lines,
                });
            }
        }
    }
    changed_functions
}
