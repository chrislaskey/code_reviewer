use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use super::error::GroupConfigError;
use super::matcher::GroupMatcher;

/// Every group of a review, as written in the config file or sent by the UI.
///
/// ```toml
/// [groups.data]
/// label = "Data"
/// include = ["core/src/storage/**"]
/// exclude = ["**/*_test.*"]
/// ```
///
/// Groups keep the order they were written in.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupConfig {
    #[serde(default)]
    pub groups: IndexMap<String, GroupDefinition>,
}

/// One group: the files that match one of its `include` patterns and none of its
/// `exclude` patterns.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupDefinition {
    /// Shown instead of the name. Defaults to the name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Globs, or `regex:` expressions, matched against repo-relative paths.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub include: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclude: Vec<String>,
}

/// Which pattern list of a group to add to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PatternKind {
    Include,
    Exclude,
}

impl GroupConfig {
    /// Parse the TOML config file contents. Unknown keys are errors, to catch typos.
    ///
    /// `source_name` is only used in error messages, for example the file path.
    pub fn from_toml_str(contents: &str, source_name: &str) -> Result<Self, GroupConfigError> {
        toml::from_str(contents).map_err(|error| GroupConfigError::ParseFailed {
            source_name: source_name.to_string(),
            message: error.to_string(),
        })
    }

    /// Check every name and pattern, and build the file matcher.
    pub fn compile(&self) -> Result<GroupMatcher, GroupConfigError> {
        GroupMatcher::compile(self)
    }

    /// Add a pattern to a group, creating the group when missing.
    pub(super) fn add_pattern(&mut self, group_name: &str, kind: PatternKind, pattern: String) {
        let group = self.groups.entry(group_name.to_string()).or_default();
        match kind {
            PatternKind::Include => group.include.push(pattern),
            PatternKind::Exclude => group.exclude.push(pattern),
        }
    }
}

/// Group names are used in file keys, flags and ids, so keep them to plain slugs.
pub(super) fn validate_group_name(name: &str) -> Result<(), GroupConfigError> {
    let reason = if name.is_empty() {
        Some("name is empty")
    } else if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        Some("use only letters, digits, `_` and `-`; set `label` for display text")
    } else {
        None
    };
    match reason {
        Some(reason) => Err(GroupConfigError::InvalidGroupName {
            name: name.to_string(),
            reason: reason.to_string(),
        }),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE: &str = r#"
        [groups.web]
        label = "Core web"
        include = ["web/**"]

        [groups.data]
        include = ["core/src/storage/**"]
        exclude = ["**/*_test.*"]
    "#;

    #[test]
    fn test_from_toml_str_reads_groups_in_file_order() {
        let config = GroupConfig::from_toml_str(EXAMPLE, "example").unwrap();

        let names: Vec<&String> = config.groups.keys().collect();
        assert_eq!(names, ["web", "data"]);
        assert_eq!(config.groups["web"].label.as_deref(), Some("Core web"));
        assert_eq!(config.groups["data"].exclude, ["**/*_test.*"]);
    }

    #[test]
    fn test_from_toml_str_rejects_subgroups() {
        let toml = "[groups.core.subgroups.data]\ninclude = [\"x\"]";

        let result = GroupConfig::from_toml_str(toml, "example");

        assert!(matches!(result, Err(GroupConfigError::ParseFailed { .. })));
    }

    #[test]
    fn test_from_toml_str_rejects_unknown_keys() {
        let result = GroupConfig::from_toml_str("[groups.a]\nincludes = [\"x\"]", "example");

        assert!(matches!(result, Err(GroupConfigError::ParseFailed { .. })));
    }

    #[test]
    fn test_from_toml_str_rejects_a_misspelled_top_level_table() {
        let result = GroupConfig::from_toml_str("[group.a]\ninclude = [\"x\"]", "example");

        assert!(matches!(result, Err(GroupConfigError::ParseFailed { .. })));
    }

    #[test]
    fn test_json_from_the_ui_describes_the_same_config_as_toml() {
        let from_toml = GroupConfig::from_toml_str(EXAMPLE, "example").unwrap();

        let json = serde_json::to_string(&from_toml).unwrap();
        let from_json: GroupConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(from_toml, from_json);
        assert!(json.find("\"web\"") < json.find("\"data\""));
    }

    #[test]
    fn test_validate_group_name_accepts_slugs_only() {
        assert!(validate_group_name("core_data-2").is_ok());
        assert!(validate_group_name("").is_err());
        assert!(validate_group_name("core data").is_err());
        assert!(validate_group_name("core/data").is_err());
        assert!(validate_group_name("a=b").is_err());
    }
}
