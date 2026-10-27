//! File groups: named sets of files such as "data" or "tests".
//!
//! Groups are defined by a config file, by command line flags, or by the UI
//! (which sends the same `GroupConfig` as JSON). A file can belong to many
//! groups. See `docs/plans/file-groups.md` for the design.

pub mod config;
pub mod error;
pub mod flags;
pub mod matcher;
mod pattern;

pub use config::{GroupConfig, GroupDefinition};
pub use error::GroupConfigError;
pub use flags::{load_group_config, GroupFlags, DEFAULT_CONFIG_FILE_NAME};
pub use matcher::{GroupMatcher, GroupSummary};
