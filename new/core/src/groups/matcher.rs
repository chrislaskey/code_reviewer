use serde::{Deserialize, Serialize};

use super::config::{validate_group_name, GroupConfig, GroupDefinition};
use super::error::GroupConfigError;
use super::pattern::PathPattern;

/// The id of the implicit group for files that no group holds.
///
/// Only exists when at least one group is defined. The name is reserved.
pub const UNGROUPED_GROUP_ID: &str = "ungrouped";

const UNGROUPED_GROUP_LABEL: &str = "Default group";

/// One group in the ordered list the UI draws.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupSummary {
    /// The name in the config, like `data`.
    pub id: String,
    /// The label, or the name.
    pub label: String,
}

/// A checked `GroupConfig`, ready to place files into groups.
///
/// Files that no group holds are in the implicit default group, so with no
/// groups defined every file is in it.
#[derive(Debug)]
pub struct GroupMatcher {
    groups: Vec<CompiledGroup>,
}

#[derive(Debug)]
struct CompiledGroup {
    id: String,
    label: String,
    include: Vec<PathPattern>,
    exclude: Vec<PathPattern>,
}

impl CompiledGroup {
    fn holds_file(&self, file_path: &str) -> bool {
        self.include
            .iter()
            .any(|pattern| pattern.is_match(file_path))
            && !self
                .exclude
                .iter()
                .any(|pattern| pattern.is_match(file_path))
    }
}

impl GroupMatcher {
    /// Check every name and pattern.
    ///
    /// - A group needs at least one `include` pattern.
    /// - The name `ungrouped` is reserved.
    pub fn compile(config: &GroupConfig) -> Result<Self, GroupConfigError> {
        let groups = config
            .groups
            .iter()
            .map(|(name, definition)| compile_group(name, definition))
            .collect::<Result<_, _>>()?;
        Ok(Self { groups })
    }

    /// Every group in config order, ending with the implicit "Default group".
    pub fn list_groups(&self) -> Vec<GroupSummary> {
        let mut summaries: Vec<GroupSummary> = self
            .groups
            .iter()
            .map(|group| GroupSummary {
                id: group.id.clone(),
                label: group.label.clone(),
            })
            .collect();
        summaries.push(GroupSummary {
            id: UNGROUPED_GROUP_ID.to_string(),
            label: UNGROUPED_GROUP_LABEL.to_string(),
        });
        summaries
    }

    /// The ids of every group that holds this file, in config order.
    ///
    /// A file in no group gets `["ungrouped"]`. `file_path` is relative to the
    /// repo root, with `/` separators.
    pub fn list_group_ids_for_file(&self, file_path: &str) -> Vec<String> {
        let mut ids: Vec<String> = self
            .groups
            .iter()
            .filter(|group| group.holds_file(file_path))
            .map(|group| group.id.clone())
            .collect();
        if ids.is_empty() {
            ids.push(UNGROUPED_GROUP_ID.to_string());
        }
        ids
    }
}

fn compile_group(
    name: &str,
    definition: &GroupDefinition,
) -> Result<CompiledGroup, GroupConfigError> {
    validate_group_name(name)?;
    if name == UNGROUPED_GROUP_ID {
        return Err(GroupConfigError::ReservedGroupName(name.to_string()));
    }
    if definition.include.is_empty() {
        return Err(GroupConfigError::EmptyGroup(name.to_string()));
    }
    Ok(CompiledGroup {
        id: name.to_string(),
        label: definition.label.clone().unwrap_or_else(|| name.to_string()),
        include: compile_patterns(name, &definition.include)?,
        exclude: compile_patterns(name, &definition.exclude)?,
    })
}

fn compile_patterns(
    group_id: &str,
    patterns: &[String],
) -> Result<Vec<PathPattern>, GroupConfigError> {
    patterns
        .iter()
        .map(|pattern| {
            PathPattern::compile(pattern).map_err(|message| GroupConfigError::InvalidPattern {
                group: group_id.to_string(),
                pattern: pattern.clone(),
                message,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matcher(toml: &str) -> GroupMatcher {
        GroupConfig::from_toml_str(toml, "test")
            .unwrap()
            .compile()
            .unwrap()
    }

    fn ids(matcher: &GroupMatcher, file_path: &str) -> Vec<String> {
        matcher.list_group_ids_for_file(file_path)
    }

    const SHOP: &str = r#"
        [groups.data]
        include = ["core/src/storage/**"]
        exclude = ["**/*_test.*"]

        [groups.analysis]
        include = ["core/src/analysis/**"]

        [groups.tests]
        include = ["**/*_test.*", "**/tests/**"]
    "#;

    #[test]
    fn test_a_file_can_belong_to_many_groups() {
        let matcher = matcher(
            r#"
            [groups.core]
            include = ["core/**"]

            [groups.tests]
            include = ["**/*_test.*"]
            "#,
        );

        assert_eq!(ids(&matcher, "core/orders_test.rs"), ["core", "tests"]);
    }

    #[test]
    fn test_exclude_takes_a_file_out_of_a_group() {
        let matcher = matcher(SHOP);

        assert_eq!(ids(&matcher, "core/src/storage/orders.rs"), ["data"]);
        assert_eq!(ids(&matcher, "core/src/analysis/model.rs"), ["analysis"]);
        assert_eq!(ids(&matcher, "core/src/storage/orders_test.rs"), ["tests"]);
    }

    #[test]
    fn test_files_in_no_group_go_to_ungrouped_once_a_group_exists() {
        let matcher = matcher(SHOP);

        assert_eq!(ids(&matcher, "README.md"), ["ungrouped"]);
        let listed = matcher.list_groups();
        assert_eq!(listed.last().map(|g| g.id.as_str()), Some("ungrouped"));
        assert_eq!(
            listed.last().map(|g| g.label.as_str()),
            Some("Default group")
        );
    }

    #[test]
    fn test_with_no_groups_every_file_is_in_the_default_group() {
        let matcher = matcher("");

        let listed: Vec<String> = matcher.list_groups().into_iter().map(|g| g.id).collect();
        assert_eq!(listed, ["ungrouped"]);
        assert_eq!(ids(&matcher, "README.md"), ["ungrouped"]);
    }

    #[test]
    fn test_ungrouped_is_a_reserved_name() {
        let config =
            GroupConfig::from_toml_str("[groups.ungrouped]\ninclude = [\"x\"]", "test").unwrap();

        assert!(matches!(
            config.compile(),
            Err(GroupConfigError::ReservedGroupName(_))
        ));
    }

    #[test]
    fn test_patterns_without_a_slash_only_match_at_the_top_level() {
        let matcher = matcher("[groups.rust]\ninclude = [\"*.rs\"]");

        assert_eq!(ids(&matcher, "main.rs"), ["rust"]);
        assert_eq!(ids(&matcher, "src/main.rs"), ["ungrouped"]);
    }

    #[test]
    fn test_list_groups_keeps_config_order_and_defaults_the_label() {
        let matcher = matcher(
            r#"
            [groups.web]
            include = ["web/**"]

            [groups.core]
            label = "Core"
            include = ["core/**"]
            "#,
        );

        let summaries: Vec<(String, String)> = matcher
            .list_groups()
            .into_iter()
            .map(|group| (group.id, group.label))
            .collect();
        assert_eq!(
            summaries,
            [
                ("web".to_string(), "web".to_string()),
                ("core".to_string(), "Core".to_string()),
                ("ungrouped".to_string(), "Default group".to_string()),
            ]
        );
    }

    #[test]
    fn test_compile_rejects_a_group_without_patterns() {
        let config = GroupConfig::from_toml_str("[groups.docs]\nlabel = \"Docs\"", "test").unwrap();

        assert!(matches!(
            config.compile(),
            Err(GroupConfigError::EmptyGroup(id)) if id == "docs"
        ));
    }

    #[test]
    fn test_compile_names_the_group_with_a_bad_pattern() {
        let config =
            GroupConfig::from_toml_str("[groups.a]\ninclude = [\"regex:(\"]", "test").unwrap();

        assert!(matches!(
            config.compile(),
            Err(GroupConfigError::InvalidPattern { group, .. }) if group == "a"
        ));
    }

    #[test]
    fn test_compile_rejects_names_that_are_not_slugs() {
        let config =
            GroupConfig::from_toml_str("[groups.\"core data\"]\ninclude = [\"x\"]", "test")
                .unwrap();

        assert!(matches!(
            config.compile(),
            Err(GroupConfigError::InvalidGroupName { .. })
        ));
    }
}
