use thiserror::Error;

/// Errors from reading, merging and compiling group definitions.
#[derive(Debug, Error)]
pub enum GroupConfigError {
    #[error("failed to read group config {path}: {message}")]
    ReadFileFailed { path: String, message: String },

    #[error("failed to parse group config {source_name}: {message}")]
    ParseFailed {
        source_name: String,
        message: String,
    },

    #[error("invalid group name {name:?}: {reason}")]
    InvalidGroupName { name: String, reason: String },

    #[error("invalid pattern {pattern:?} in group {group}: {message}")]
    InvalidPattern {
        group: String,
        pattern: String,
        message: String,
    },

    #[error("group {0} has no include patterns and no subgroups")]
    EmptyGroup(String),

    #[error("group {0} has subgroups, so it cannot have include or exclude patterns of its own")]
    ParentGroupHasPatterns(String),

    #[error("{0:?} is reserved for files that are in no group")]
    ReservedGroupName(String),

    #[error("invalid group flag {flag:?}: {reason}")]
    InvalidFlag { flag: String, reason: String },
}
