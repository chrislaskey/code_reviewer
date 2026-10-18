use serde::{Deserialize, Serialize};

use super::config::{validate_group_name, GroupConfig, GroupDefinition};
use super::error::GroupConfigError;
use super::pattern::PathPattern;

/// The id of the implicit group for files that no group holds.
///
/// Only exists when at least one group is defined. The name is reserved.
pub const UNGROUPED_GROUP_ID: &str = "ungrouped";

const UNGROUPED_GROUP_LABEL: &str = "Ungrouped";

/// One group in the flat, ordered list the UI draws as a tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupSummary {
    /// The names from the top group down, joined by `/`, like `core/data`.
    pub id: String,
    /// The id of the group this one is nested in. `None` for top-level groups.
    pub parent_id: Option<String>,
    /// The label, or the last name in the id.
    pub label: String,
    /// Subgroups roll up into it and it has no patterns of its own.
    pub is_parent: bool,
}

/// A checked `GroupConfig`, ready to place files into groups.
///
/// Groups are opt in. With no groups defined nothing is grouped: no group is
/// listed and every file gets an empty list of ids, with no "Ungrouped".
#[derive(Debug)]
pub struct GroupMatcher {
    groups: Vec<CompiledGroup>,
}

#[derive(Debug)]
struct CompiledGroup {
    id: String,
    parent_id: Option<String>,
    label: String,
    kind: CompiledGroupKind,
}

#[derive(Debug)]
enum CompiledGroupKind {
    /// Holds the files matching its patterns.
    Leaf {
        include: Vec<PathPattern>,
        exclude: Vec<PathPattern>,
    },
    /// Holds the files of its subgroups, and nothing else.
    Parent { subgroups: Vec<CompiledGroup> },
}

impl GroupMatcher {
    /// Check every name and pattern.
    ///
    /// - A group with subgroups is a pure roll-up and cannot have `include` or `exclude`.
    /// - A group without subgroups needs at least one `include` pattern.
    /// - The top-level name `ungrouped` is reserved.
    pub fn compile(config: &GroupConfig) -> Result<Self, GroupConfigError> {
        let groups = config
            .groups
            .iter()
            .map(|(name, definition)| compile_group(name, definition, None))
            .collect::<Result<_, _>>()?;
        Ok(Self { groups })
    }

    /// True when at least one group is defined, so files should be grouped.
    pub fn has_groups(&self) -> bool {
        !self.groups.is_empty()
    }

    /// Every group, parents before their subgroups, in config order.
    ///
    /// Ends with the implicit "Ungrouped" group when any group is defined.
    pub fn list_groups(&self) -> Vec<GroupSummary> {
        if !self.has_groups() {
            return Vec::new();
        }
        let mut summaries = Vec::new();
        for group in &self.groups {
            collect_summaries(group, &mut summaries);
        }
        summaries.push(GroupSummary {
            id: UNGROUPED_GROUP_ID.to_string(),
            parent_id: None,
            label: UNGROUPED_GROUP_LABEL.to_string(),
            is_parent: false,
        });
        summaries
    }

    /// The ids of every group that holds this file, parents before subgroups.
    ///
    /// A file in no group gets `["ungrouped"]`, unless no groups are defined at
    /// all, which gives an empty list. `file_path` is relative to the repo
    /// root, with `/` separators.
    pub fn list_group_ids_for_file(&self, file_path: &str) -> Vec<String> {
        if !self.has_groups() {
            return Vec::new();
        }
        let mut ids = Vec::new();
        for group in &self.groups {
            collect_group_ids(group, file_path, &mut ids);
        }
        if ids.is_empty() {
            ids.push(UNGROUPED_GROUP_ID.to_string());
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
    if parent_id.is_none() && name == UNGROUPED_GROUP_ID {
        return Err(GroupConfigError::ReservedGroupName(name.to_string()));
    }
    let id = match parent_id {
        Some(parent_id) => format!("{parent_id}/{name}"),
        None => name.to_string(),
    };

    let kind = if definition.subgroups.is_empty() {
        if definition.include.is_empty() {
            return Err(GroupConfigError::EmptyGroup(id));
        }
        CompiledGroupKind::Leaf {
            include: compile_patterns(&id, &definition.include)?,
            exclude: compile_patterns(&id, &definition.exclude)?,
        }
    } else {
        if !definition.include.is_empty() || !definition.exclude.is_empty() {
            return Err(GroupConfigError::ParentGroupHasPatterns(id));
        }
        CompiledGroupKind::Parent {
            subgroups: definition
                .subgroups
                .iter()
                .map(|(name, subgroup)| compile_group(name, subgroup, Some(&id)))
                .collect::<Result<_, _>>()?,
        }
    };

    Ok(CompiledGroup {
        label: definition.label.clone().unwrap_or_else(|| name.to_string()),
        parent_id: parent_id.map(str::to_string),
        id,
        kind,
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
        is_parent: matches!(group.kind, CompiledGroupKind::Parent { .. }),
    });
    if let CompiledGroupKind::Parent { subgroups } = &group.kind {
        for subgroup in subgroups {
            collect_summaries(subgroup, summaries);
        }
    }
}

/// Pushes this group and its subgroups when they hold the file. Returns whether this group does.
fn collect_group_ids(group: &CompiledGroup, file_path: &str, ids: &mut Vec<String>) -> bool {
    match &group.kind {
        CompiledGroupKind::Leaf { include, exclude } => {
            let holds_file = include.iter().any(|pattern| pattern.is_match(file_path))
                && !exclude.iter().any(|pattern| pattern.is_match(file_path));
            if holds_file {
                ids.push(group.id.clone());
            }
            holds_file
        }
        CompiledGroupKind::Parent { subgroups } => {
            let mut subgroup_ids = Vec::new();
            for subgroup in subgroups {
                // Visit every subgroup: each one adds its own ids.
                collect_group_ids(subgroup, file_path, &mut subgroup_ids);
            }
            let holds_file = !subgroup_ids.is_empty();
            if holds_file {
                ids.push(group.id.clone());
                ids.extend(subgroup_ids);
            }
            holds_file
        }
    }
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
        [groups.core.subgroups.data]
        include = ["core/src/storage/**"]
        exclude = ["**/*_test.*"]

        [groups.core.subgroups.analysis]
        include = ["core/src/analysis/**"]

        [groups.tests]
        include = ["**/*_test.*", "**/tests/**"]
    "#;

    #[test]
    fn test_a_file_can_belong_to_many_groups() {
        let matcher = matcher(
            r#"
            [groups.core.subgroups.data]
            include = ["core/**"]

            [groups.tests]
            include = ["**/*_test.*"]
            "#,
        );

        assert_eq!(
            ids(&matcher, "core/orders_test.rs"),
            ["core", "core/data", "tests"]
        );
    }

    #[test]
    fn test_parent_holds_what_its_subgroups_hold() {
        let matcher = matcher(SHOP);

        assert_eq!(
            ids(&matcher, "core/src/storage/orders.rs"),
            ["core", "core/data"]
        );
        assert_eq!(
            ids(&matcher, "core/src/analysis/model.rs"),
            ["core", "core/analysis"]
        );
        assert_eq!(ids(&matcher, "core/src/storage/orders_test.rs"), ["tests"]);
    }

    #[test]
    fn test_files_in_no_group_go_to_ungrouped_once_a_group_exists() {
        let matcher = matcher(SHOP);

        assert_eq!(ids(&matcher, "README.md"), ["ungrouped"]);
        let listed = matcher.list_groups();
        assert_eq!(listed.last().map(|g| g.id.as_str()), Some("ungrouped"));
        assert_eq!(listed.last().map(|g| g.label.as_str()), Some("Ungrouped"));
    }

    #[test]
    fn test_with_no_groups_nothing_is_grouped() {
        let matcher = matcher("");

        assert!(!matcher.has_groups());
        assert!(matcher.list_groups().is_empty());
        assert!(ids(&matcher, "README.md").is_empty());
    }

    #[test]
    fn test_a_parent_cannot_have_include_or_exclude() {
        for field in ["include", "exclude"] {
            let config = GroupConfig::from_toml_str(
                &format!(
                    "[groups.core]\n{field} = [\"core/**\"]\n[groups.core.subgroups.data]\ninclude = [\"core/data/**\"]"
                ),
                "test",
            )
            .unwrap();

            assert!(
                matches!(
                    config.compile(),
                    Err(GroupConfigError::ParentGroupHasPatterns(id)) if id == "core"
                ),
                "{field}"
            );
        }
    }

    #[test]
    fn test_ungrouped_is_a_reserved_top_level_name() {
        let config =
            GroupConfig::from_toml_str("[groups.ungrouped]\ninclude = [\"x\"]", "test").unwrap();
        assert!(matches!(
            config.compile(),
            Err(GroupConfigError::ReservedGroupName(_))
        ));

        let nested = GroupConfig::from_toml_str(
            "[groups.core.subgroups.ungrouped]\ninclude = [\"x\"]",
            "test",
        )
        .unwrap();
        assert!(nested.compile().is_ok());
    }

    #[test]
    fn test_patterns_without_a_slash_only_match_at_the_top_level() {
        let matcher = matcher("[groups.rust]\ninclude = [\"*.rs\"]");

        assert_eq!(ids(&matcher, "main.rs"), ["rust"]);
        assert_eq!(ids(&matcher, "src/main.rs"), ["ungrouped"]);
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

        let summaries: Vec<(String, Option<String>, String, bool)> = matcher
            .list_groups()
            .into_iter()
            .map(|group| (group.id, group.parent_id, group.label, group.is_parent))
            .collect();
        assert_eq!(
            summaries,
            [
                ("web".to_string(), None, "web".to_string(), false),
                ("core".to_string(), None, "Core".to_string(), true),
                (
                    "core/data".to_string(),
                    Some("core".to_string()),
                    "data".to_string(),
                    false
                ),
                (
                    "ungrouped".to_string(),
                    None,
                    "Ungrouped".to_string(),
                    false
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
