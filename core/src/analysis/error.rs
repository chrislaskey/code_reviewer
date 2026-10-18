use thiserror::Error;

use crate::git::GitError;
use crate::groups::GroupConfigError;

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
}
