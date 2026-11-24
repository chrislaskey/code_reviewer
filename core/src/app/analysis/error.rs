use thiserror::Error;

use crate::app::groups::GroupConfigError;
use crate::services::git::GitError;

/// Errors from indexing and analyzing source code.
#[derive(Debug, Error)]
pub enum AnalysisError {
    #[error(transparent)]
    Git(#[from] GitError),

    #[error(transparent)]
    Groups(#[from] GroupConfigError),

    #[error("failed to read {path}: {message}")]
    ReadFileFailed { path: String, message: String },

    #[error("failed to set up the {language} parser: {message}")]
    ParserSetupFailed { language: String, message: String },

    #[error("failed to parse {path}")]
    ParseFailed { path: String },

    #[error("function not found in the project index: {0}")]
    FunctionNotFound(String),

    #[error("a function diff needs an old function, a new function, or both")]
    EmptyFunctionDiffTarget,

    #[error("a file diff needs an old path, a new path, or both")]
    EmptyFileDiffTarget,
}
