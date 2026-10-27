use serde::{Deserialize, Serialize};

use super::error::AnalysisError;
use super::model::{FunctionDefinition, FunctionId};
use super::project_index::ProjectIndex;

/// One numbered line of source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceLine {
    pub number: usize,
    pub text: String,
}

/// The complete source of a function: leading `@doc`/`@spec`/`@impl`, every clause, to the last `end`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionSource {
    pub function: FunctionId,
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub lines: Vec<SourceLine>,
}

/// Cut the lines of `definition` out of `file_text`. Lines past the end of the text are dropped.
pub fn extract_function_source(definition: &FunctionDefinition, file_text: &str) -> FunctionSource {
    let lines = file_text
        .lines()
        .enumerate()
        .map(|(offset, text)| SourceLine {
            number: offset + 1,
            text: text.to_string(),
        })
        .filter(|line| (definition.start_line..=definition.end_line).contains(&line.number))
        .collect();

    FunctionSource {
        function: definition.id.clone(),
        file_path: definition.file_path.clone(),
        start_line: definition.start_line,
        end_line: definition.end_line,
        lines,
    }
}

/// Read the full source of `function` from the snapshot the index was built from.
pub fn get_function_source(
    index: &ProjectIndex,
    function: &FunctionId,
) -> Result<FunctionSource, AnalysisError> {
    let definition = index
        .get_function(function)
        .ok_or_else(|| AnalysisError::FunctionNotFound(function.to_string()))?;

    let file_text = index.read_file_text(&definition.file_path)?;
    Ok(extract_function_source(definition, &file_text))
}
