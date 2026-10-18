use serde::{Deserialize, Serialize};

use super::config::{validate_group_name, GroupConfig, GroupDefinition};
use super::error::GroupConfigError;
use super::pattern::PathPattern;

/// One group in the flat, ordered list the UI draws as a tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupSummary {
    /// The names from the top group down, joined by `/`, like `core/data`.
    pub id: String,
    /// The id of the group this one is nested in. `None` for top-level groups.
    pub parent_id: Option<String>,
    /// The label, or the last name in the id.
    pub label: String,
}

/// A checked `GroupConfig`, ready to place files into groups.
#[derive(Debug)]
pub struct GroupMatcher {
    groups: Vec<CompiledGroup>,
}

#[derive(Debug)]
struct CompiledGroup {
    id: String,
    parent_id: Option<String>,
    label: String,
    include: Vec<PathPattern>,
    exclude: Vec<PathPattern>,
    subgroups: Vec<CompiledGroup>,
}

impl GroupMatcher {
    /// Check every name and pattern. A group needs include patterns or subgroups.
    pub fn compile(config: &GroupConfig) -> Result<Self, GroupConfigError> {
        let groups = config
            .groups
            .iter()
            .map(|(name, definition)| compile_group(name, definition, None))
            .collect::<Result<_, _>>()?;
        Ok(Self { groups })
    }

    /// Every group, parents before their subgroups, in config order.
    pub fn list_groups(&self) -> Vec<GroupSummary> {
        let mut summaries = Vec::new();
        for group in &self.groups {
            collect_summaries(group, &mut summaries);
        }
        summaries
    }

    /// The ids of every group that holds this file, parents before subgroups.
    ///
    /// `file_path` is relative to the repo root, with `/` separators.
    pub fn list_group_ids_for_file(&self, file_path: &str) -> Vec<String> {
        let mut ids = Vec::new();
        for group in &self.groups {
            collect_group_ids(group, file_path, &mut ids);
        }
        ids
    }
}

fn compile_group(
    name: &str,
    definition: &GroupDefinition,
    parent_id: Option<&str>,
) -> Result<CompiledGroup, GroupConfigError> {
    validate_group_name(name)?;
    let id = match parent_id {
        Some(parent_id) => format!("{parent_id}/{name}"),
        None => name.to_string(),
    };
    if definition.include.is_empty() && definition.subgroups.is_empty() {
        return Err(GroupConfigError::EmptyGroup(id));
    }
    Ok(CompiledGroup {
        label: definition.label.clone().unwrap_or_else(|| name.to_string()),
        include: compile_patterns(&id, &definition.include)?,
        exclude: compile_patterns(&id, &definition.exclude)?,
        subgroups: definition
            .subgroups
            .iter()
            .map(|(name, subgroup)| compile_group(name, subgroup, Some(&id)))
            .collect::<Result<_, _>>()?,
        parent_id: parent_id.map(str::to_string),
        id,
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

fn collect_summaries(group: &CompiledGroup, summaries: &mut Vec<GroupSummary>) {
    summaries.push(GroupSummary {
        id: group.id.clone(),
        parent_id: group.parent_id.clone(),
        label: group.label.clone(),
    });
    for subgroup in &group.subgroups {
        collect_summaries(subgroup, summaries);
    }
}

/// Pushes this group and its subgroups when they hold the file. Returns whether this group does.
fn collect_group_ids(group: &CompiledGroup, file_path: &str, ids: &mut Vec<String>) -> bool {
    let mut subgroup_ids = Vec::new();
    let mut holds_through_subgroup = false;
    for subgroup in &group.subgroups {
        // Visit every subgroup: each one adds its own ids.
        if collect_group_ids(subgroup, file_path, &mut subgroup_ids) {
            holds_through_subgroup = true;
        }
    }

    let holds_directly = group
        .include
        .iter()
        .any(|pattern| pattern.is_match(file_path))
        && !group
            .exclude
            .iter()
            .any(|pattern| pattern.is_match(file_path));

    let holds_file = holds_directly || holds_through_subgroup;
    if holds_file {
        ids.push(group.id.clone());
        ids.extend(subgroup_ids);
    }
    holds_file
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
        [groups.core]
        include = ["core/src/lib.rs"]

        [groups.core.subgroups.data]
        include = ["core/src/storage/**"]
        exclude = ["**/*_test.*"]

        [groups.tests]
        include = ["**/*_test.*", "**/tests/**"]
    "#;

    #[test]
    fn test_a_file_can_belong_to_many_groups() {
        let matcher = matcher(SHOP);

        assert_eq!(ids(&matcher, "core/src/storage/orders_test.rs"), ["tests"]);
        assert_eq!(
            ids(&matcher, "core/src/storage/orders.rs"),
            ["core", "core/data"]
        );
        assert_eq!(ids(&matcher, "core/src/lib.rs"), ["core"]);
        assert!(ids(&matcher, "README.md").is_empty());
    }

    #[test]
    fn test_parent_holds_what_its_subgroups_hold() {
        let matcher = matcher(
            r#"
            [groups.core.subgroups.data]
            include = ["core/storage/**"]
            "#,
        );

        assert_eq!(ids(&matcher, "core/storage/a.rs"), ["core", "core/data"]);
    }

    #[test]
    fn test_parent_exclude_does_not_remove_a_file_from_a_subgroup() {
        let matcher = matcher(
            r#"
            [groups.core]
            include = ["core/**"]
            exclude = ["core/storage/**"]

            [groups.core.subgroups.data]
            include = ["core/storage/**"]
            "#,
        );

        assert_eq!(ids(&matcher, "core/storage/a.rs"), ["core", "core/data"]);
        assert_eq!(ids(&matcher, "core/other.rs"), ["core"]);
    }

    #[test]
    fn test_list_groups_orders_parents_first_and_defaults_the_label() {
        let matcher = matcher(
            r#"
            [groups.web]
            include = ["web/**"]

            [groups.core]
            label = "Core"
            [groups.core.subgroups.data]
            include = ["core/storage/**"]
            "#,
        );

        let summaries: Vec<(String, Option<String>, String)> = matcher
            .list_groups()
            .into_iter()
            .map(|group| (group.id, group.parent_id, group.label))
            .collect();
        assert_eq!(
            summaries,
            [
                ("web".to_string(), None, "web".to_string()),
                ("core".to_string(), None, "Core".to_string()),
                (
                    "core/data".to_string(),
                    Some("core".to_string()),
                    "data".to_string()
                ),
            ]
        );
    }

    #[test]
    fn test_compile_rejects_a_group_without_patterns_or_subgroups() {
        let config = GroupConfig::from_toml_str("[groups.docs]\nlabel = \"Docs\"", "test").unwrap();

        assert!(matches!(
            config.compile(),
            Err(GroupConfigError::EmptyGroup(id)) if id == "docs"
        ));
    }

    #[test]
    fn test_compile_names_the_group_with_a_bad_pattern() {
        let config =
            GroupConfig::from_toml_str("[groups.a.subgroups.b]\ninclude = [\"regex:(\"]", "test")
                .unwrap();

        assert!(matches!(
            config.compile(),
            Err(GroupConfigError::InvalidPattern { group, .. }) if group == "a/b"
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
