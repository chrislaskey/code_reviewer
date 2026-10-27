//! The changed files of one review, rolled up into groups.
//!
//! A group row is the file view one level up: a group where a file has its
//! header, the group's files where a file has its functions,
//! and the files outside the group on the other end of its calls.

use std::collections::HashMap;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use super::file_changes::{FileChange, FileChangeStatus};
use super::function_changes::FunctionChangeKind;
use crate::groups::{GroupMatcher, GroupSummary};

/// How many changed functions of each kind.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionCounts {
    pub added: usize,
    pub modified: usize,
    pub removed: usize,
    pub renamed: usize,
}

/// The numbers a group row shows for its files.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupStats {
    pub file_count: usize,
    pub function_count: usize,
    pub function_counts: FunctionCounts,
    pub added_lines: usize,
    pub removed_lines: usize,
    /// Callers still calling something removed or renamed.
    pub stale_caller_count: usize,
}

/// What a line of a group row is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupLineKind {
    File,
    Group,
}

/// One line of the content column: a changed file, or a group on the all changes row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupLine {
    /// What hovering the line ties its outputs and inputs to. The file path, or the group id.
    pub key: String,
    pub kind: GroupLineKind,
    /// The file path, or the group id.
    pub id: String,
    /// The file path, or the group label.
    pub label: String,
    pub stats: GroupStats,
    /// Files only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<FileChangeStatus>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupLinkSide {
    /// A file outside the group that calls into it.
    Input,
    /// A file outside the group that its files call.
    Output,
}

impl GroupLinkSide {
    fn as_str(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::Output => "output",
        }
    }
}

/// A file outside the group on the other end of a call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupLink {
    pub id: String,
    pub side: GroupLinkSide,
    pub path: String,
    pub module: String,
    /// The content line this link belongs to.
    pub line_key: String,
    pub line_label: String,
    /// The most specific group of the other file.
    pub group_id: String,
    pub group_label: String,
    /// The path again, when the other file is itself changed in this review.
    pub changed_file_path: Option<String>,
    pub call_count: usize,
    /// Inputs only: still calls something removed or renamed.
    pub is_stale: bool,
}

/// What the content column of a row lists, as a label.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GroupContentLabel {
    Files,
    Groups,
}

/// One group, drawn as a row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupRow {
    pub group: GroupSummary,
    pub stats: GroupStats,
    pub status: FileChangeStatus,
    pub content_label: GroupContentLabel,
    pub lines: Vec<GroupLine>,
    pub outputs: Vec<GroupLink>,
    pub inputs: Vec<GroupLink>,
    /// Calls into dependencies and the standard library, left out of the outputs.
    pub folded_output_count: usize,
}

/// The groups that hold changed files, and a row for each.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupedChanges {
    /// In config order, ending with the default group.
    pub groups: Vec<GroupSummary>,
    pub rows: Vec<GroupRow>,
    /// One row above every group.
    pub all_changes_row: GroupRow,
}

/// The id of the all changes row. A group name cannot contain `@`, so it never clashes.
pub const ALL_CHANGES_GROUP_ID: &str = "@all";

/// Set the group ids of every file, then build a row for each group that holds a changed file.
///
/// With no groups defined every file is in the default group.
pub fn group_changed_files(files: &mut [FileChange], matcher: &GroupMatcher) -> GroupedChanges {
    for file in files.iter_mut() {
        file.group_ids = matcher.list_group_ids_for_file(&file.file_path);
    }
    let files: &[FileChange] = files;

    let all_groups = matcher.list_groups();
    let groups: Vec<GroupSummary> = all_groups
        .iter()
        .filter(|group| files.iter().any(|file| file.group_ids.contains(&group.id)))
        .cloned()
        .collect();

    let mut path_groups = PathGroups::new(matcher);
    let rows = groups
        .iter()
        .map(|group| build_group_row(group, &all_groups, files, &mut path_groups))
        .collect();
    let all_changes_row = build_all_changes_row(&groups, files);
    GroupedChanges {
        groups,
        rows,
        all_changes_row,
    }
}

/// The group ids of paths, worked out once per path.
struct PathGroups<'a> {
    matcher: &'a GroupMatcher,
    cache: HashMap<String, Vec<String>>,
}

impl<'a> PathGroups<'a> {
    fn new(matcher: &'a GroupMatcher) -> Self {
        Self {
            matcher,
            cache: HashMap::new(),
        }
    }

    fn list_group_ids(&mut self, path: &str) -> &[String] {
        self.cache
            .entry(path.to_string())
            .or_insert_with(|| self.matcher.list_group_ids_for_file(path))
    }
}

fn summarize_files(files: &[&FileChange]) -> GroupStats {
    let mut stats = GroupStats {
        file_count: files.len(),
        ..GroupStats::default()
    };
    for file in files {
        for change in &file.functions {
            stats.function_count += 1;
            let counter = match change.kind {
                FunctionChangeKind::Added => &mut stats.function_counts.added,
                FunctionChangeKind::Modified => &mut stats.function_counts.modified,
                FunctionChangeKind::Removed => &mut stats.function_counts.removed,
                FunctionChangeKind::Renamed => &mut stats.function_counts.renamed,
            };
            *counter += 1;
            stats.added_lines += change.added_line_count.unwrap_or(0);
            stats.removed_lines += change.removed_line_count.unwrap_or(0);
        }
        stats.stale_caller_count += file.callers.iter().filter(|caller| caller.is_stale).count();
    }
    stats
}

fn group_status(stats: &GroupStats) -> FileChangeStatus {
    let counts = &stats.function_counts;
    if counts.added > 0 && counts.modified + counts.removed + counts.renamed == 0 {
        FileChangeStatus::Added
    } else if counts.removed > 0 && counts.added + counts.modified + counts.renamed == 0 {
        FileChangeStatus::Removed
    } else {
        FileChangeStatus::Modified
    }
}

fn list_files_in_group<'a>(files: &'a [FileChange], group_id: &str) -> Vec<&'a FileChange> {
    files
        .iter()
        .filter(|file| file.group_ids.iter().any(|id| id == group_id))
        .collect()
}

/// The first group in the list that holds the path.
fn find_first_group<'a>(
    ids: &[String],
    all_groups: &'a [GroupSummary],
) -> Option<&'a GroupSummary> {
    all_groups.iter().find(|group| ids.contains(&group.id))
}

struct LinkCollector<'a> {
    all_groups: &'a [GroupSummary],
    files: &'a [FileChange],
    lines: &'a [GroupLine],
    links: IndexMap<String, GroupLink>,
}

impl LinkCollector<'_> {
    fn add(
        &mut self,
        side: GroupLinkSide,
        line_key: &str,
        other: OtherFile<'_>,
        call_count: usize,
        is_stale: bool,
    ) {
        let id = format!("{}:{}:{}", side.as_str(), line_key, other.path);
        if let Some(existing) = self.links.get_mut(&id) {
            existing.call_count += call_count;
            existing.is_stale |= is_stale;
            return;
        }
        let line_label = self
            .lines
            .iter()
            .find(|line| line.key == line_key)
            .map_or(line_key, |line| line.label.as_str())
            .to_string();
        let group = find_first_group(other.group_ids, self.all_groups);
        let is_changed = self.files.iter().any(|file| file.file_path == other.path);
        self.links.insert(
            id.clone(),
            GroupLink {
                id,
                side,
                path: other.path.to_string(),
                module: other.module.to_string(),
                line_key: line_key.to_string(),
                line_label,
                group_id: group.map(|group| group.id.clone()).unwrap_or_default(),
                group_label: group.map(|group| group.label.clone()).unwrap_or_default(),
                changed_file_path: is_changed.then(|| other.path.to_string()),
                call_count,
                is_stale,
            },
        );
    }

    fn list_sorted(&self, side: GroupLinkSide) -> Vec<GroupLink> {
        let line_order = |key: &str| self.lines.iter().position(|line| line.key == key);
        let mut links: Vec<GroupLink> = self
            .links
            .values()
            .filter(|link| link.side == side)
            .cloned()
            .collect();
        links.sort_by_key(|link| line_order(&link.line_key));
        links
    }
}

/// The file on the other end of a call.
struct OtherFile<'a> {
    path: &'a str,
    module: &'a str,
    group_ids: &'a [String],
}

fn build_lines(group_files: &[&FileChange]) -> Vec<GroupLine> {
    group_files
        .iter()
        .map(|file| GroupLine {
            key: file.file_path.clone(),
            kind: GroupLineKind::File,
            id: file.file_path.clone(),
            label: file.file_path.clone(),
            stats: summarize_files(&[file]),
            status: Some(file.status),
        })
        .collect()
}

/// One row above all the groups that hold files, so the whole review rolls up in one place.
fn build_all_changes_row(groups: &[GroupSummary], files: &[FileChange]) -> GroupRow {
    let all_files: Vec<&FileChange> = files.iter().collect();
    let stats = summarize_files(&all_files);
    let lines = groups
        .iter()
        .map(|group| GroupLine {
            key: group.id.clone(),
            kind: GroupLineKind::Group,
            id: group.id.clone(),
            label: group.label.clone(),
            stats: summarize_files(&list_files_in_group(files, &group.id)),
            status: None,
        })
        .collect();
    let folded_output_count = files
        .iter()
        .flat_map(|file| &file.callees)
        .filter(|callee| callee.file_path.is_none() || !callee.is_highlighted)
        .count();

    GroupRow {
        group: GroupSummary {
            id: ALL_CHANGES_GROUP_ID.to_string(),
            label: "All changes by group".to_string(),
        },
        status: group_status(&stats),
        stats,
        content_label: GroupContentLabel::Groups,
        lines,
        outputs: Vec::new(),
        inputs: Vec::new(),
        folded_output_count,
    }
}

fn build_group_row(
    group: &GroupSummary,
    all_groups: &[GroupSummary],
    files: &[FileChange],
    path_groups: &mut PathGroups<'_>,
) -> GroupRow {
    let group_files = list_files_in_group(files, &group.id);
    let stats = summarize_files(&group_files);
    let lines = build_lines(&group_files);

    let mut folded_output_count = 0;
    let mut collector = LinkCollector {
        all_groups,
        files,
        lines: &lines,
        links: IndexMap::new(),
    };
    for file in &group_files {
        let key = &file.file_path;
        for callee in &file.callees {
            let path = callee
                .file_path
                .as_deref()
                .filter(|_| callee.is_highlighted);
            let Some(path) = path else {
                folded_output_count += 1;
                continue;
            };
            let group_ids = path_groups.list_group_ids(path).to_vec();
            if group_ids.contains(&group.id) {
                continue;
            }
            let other = OtherFile {
                path,
                module: &callee.function.module,
                group_ids: &group_ids,
            };
            collector.add(GroupLinkSide::Output, key, other, callee.call_count, false);
        }
        for caller in &file.callers {
            let group_ids = path_groups.list_group_ids(&caller.file_path).to_vec();
            if group_ids.contains(&group.id) {
                continue;
            }
            let other = OtherFile {
                path: &caller.file_path,
                module: &caller.function.module,
                group_ids: &group_ids,
            };
            collector.add(GroupLinkSide::Input, key, other, 1, caller.is_stale);
        }
    }

    GroupRow {
        group: group.clone(),
        status: group_status(&stats),
        stats,
        content_label: GroupContentLabel::Files,
        outputs: collector.list_sorted(GroupLinkSide::Output),
        inputs: collector.list_sorted(GroupLinkSide::Input),
        lines,
        folded_output_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::file_changes::{FileCallee, FileCaller, FileFunctionChange};
    use crate::analysis::model::FunctionId;
    use crate::groups::GroupConfig;

    const CONFIG: &str = r#"
        [groups.data]
        label = "Data"
        include = ["lib/shop/repo/**"]

        [groups.business]
        include = ["lib/shop/*.ex"]

        [groups.web]
        include = ["lib/shop_web/**"]
    "#;

    fn matcher() -> GroupMatcher {
        GroupConfig::from_toml_str(CONFIG, "test")
            .and_then(|config| config.compile())
            .unwrap()
    }

    fn function(kind: FunctionChangeKind, module: &str, name: &str) -> FileFunctionChange {
        FileFunctionChange {
            kind,
            function: FunctionId::new(module, name, 1),
            previous_function: None,
            added_line_count: Some(3),
            removed_line_count: None,
        }
    }

    fn caller(module: &str, file_path: &str, is_stale: bool) -> FileCaller {
        FileCaller {
            function: FunctionId::new(module, "call", 0),
            file_path: file_path.to_string(),
            line: 1,
            calls: FunctionId::new("Shop.Orders", "create", 1),
            is_stale,
            is_also_changed: false,
        }
    }

    fn callee(module: &str, file_path: Option<&str>, is_highlighted: bool) -> FileCallee {
        FileCallee {
            function: FunctionId::new(module, "run", 1),
            defined_in_project: file_path.is_some(),
            is_highlighted,
            from: FunctionId::new("Shop.Orders", "create", 1),
            call_count: 2,
            is_also_changed: false,
            file_path: file_path.map(str::to_string),
        }
    }

    fn file(path: &str, module: &str) -> FileChange {
        FileChange {
            kind: Default::default(),
            previous_path: None,
            file_path: path.to_string(),
            module: module.to_string(),
            status: FileChangeStatus::Modified,
            functions: vec![function(FunctionChangeKind::Modified, module, "create")],
            callers: Vec::new(),
            callees: Vec::new(),
            group_ids: Vec::new(),
        }
    }

    fn sample_files() -> Vec<FileChange> {
        let mut orders = file("lib/shop/orders.ex", "Shop.Orders");
        orders.callers = vec![caller(
            "ShopWeb.OrderController",
            "lib/shop_web/orders.ex",
            true,
        )];
        orders.callees = vec![
            callee("Shop.Repo.Queries", Some("lib/shop/repo/queries.ex"), true),
            callee("ShopWeb.Helpers", Some("lib/shop_web/helpers.ex"), true),
            callee("Enum", None, false),
        ];
        let queries = file("lib/shop/repo/queries.ex", "Shop.Repo.Queries");
        let controller = file("lib/shop_web/orders.ex", "ShopWeb.OrderController");
        let seed = file("mix/seed.ex", "Mix.Seed");
        vec![orders, queries, controller, seed]
    }

    fn row<'a>(grouped: &'a GroupedChanges, id: &str) -> &'a GroupRow {
        grouped
            .rows
            .iter()
            .find(|row| row.group.id == id)
            .unwrap_or_else(|| panic!("no row for {id}"))
    }

    #[test]
    fn test_without_groups_every_file_is_in_the_default_group() {
        let matcher = GroupConfig::default().compile().unwrap();
        let mut files = sample_files();

        let grouped = group_changed_files(&mut files, &matcher);

        let ids: Vec<&str> = grouped.groups.iter().map(|g| g.id.as_str()).collect();
        assert_eq!(ids, ["ungrouped"]);
        assert_eq!(grouped.rows.len(), 1);
        assert_eq!(grouped.rows[0].stats.file_count, files.len());
        assert!(files.iter().all(|file| file.group_ids == ["ungrouped"]));
    }

    #[test]
    fn test_files_get_group_ids_and_only_groups_with_changes_are_listed() {
        let mut files = sample_files();

        let grouped = group_changed_files(&mut files, &matcher());

        assert_eq!(files[0].group_ids, ["business"]);
        assert_eq!(files[3].group_ids, ["ungrouped"]);
        let ids: Vec<&str> = grouped
            .groups
            .iter()
            .map(|group| group.id.as_str())
            .collect();
        assert_eq!(ids, ["data", "business", "web", "ungrouped"]);
    }

    #[test]
    fn test_every_group_with_changes_gets_a_row() {
        let mut files = sample_files();

        let grouped = group_changed_files(&mut files, &matcher());

        let row_ids: Vec<&str> = grouped
            .rows
            .iter()
            .map(|row| row.group.id.as_str())
            .collect();
        assert_eq!(row_ids, ["data", "business", "web", "ungrouped"]);
        assert_eq!(row(&grouped, "data").group.label, "Data");
        assert_eq!(row(&grouped, "web").group.label, "web");
    }

    #[test]
    fn test_the_all_changes_row_lists_every_group_that_has_files() {
        let mut files = sample_files();
        let file_count = files.len();

        let grouped = group_changed_files(&mut files, &matcher());

        let all = grouped.all_changes_row;
        assert_eq!(all.group.id, ALL_CHANGES_GROUP_ID);
        assert_eq!(all.content_label, GroupContentLabel::Groups);
        assert_eq!(all.stats.file_count, file_count);
        let line_ids: Vec<&str> = all.lines.iter().map(|line| line.id.as_str()).collect();
        assert_eq!(line_ids, ["data", "business", "web", "ungrouped"]);
        assert_eq!(all.lines[0].label, "Data");
        assert!(all.inputs.is_empty() && all.outputs.is_empty());
    }

    #[test]
    fn test_a_row_links_only_to_files_outside_the_group() {
        let mut files = sample_files();

        let grouped = group_changed_files(&mut files, &matcher());

        let business = row(&grouped, "business");
        assert_eq!(business.content_label, GroupContentLabel::Files);
        let outputs: Vec<&str> = business
            .outputs
            .iter()
            .map(|link| link.path.as_str())
            .collect();
        assert_eq!(
            outputs,
            ["lib/shop/repo/queries.ex", "lib/shop_web/helpers.ex"]
        );
        assert_eq!(business.outputs[0].group_id, "data");
        assert_eq!(business.outputs[0].group_label, "Data");
        assert_eq!(
            business.outputs[0].changed_file_path.as_deref(),
            Some("lib/shop/repo/queries.ex")
        );
        assert_eq!(business.outputs[0].call_count, 2);
        assert_eq!(business.outputs[1].changed_file_path, None);
        assert_eq!(business.folded_output_count, 1);
        assert_eq!(business.inputs.len(), 1);
        assert_eq!(business.inputs[0].group_id, "web");
        assert!(business.inputs[0].is_stale);
        assert_eq!(business.stats.stale_caller_count, 1);
    }

    #[test]
    fn test_group_status_follows_the_changed_functions() {
        let mut added = file("lib/shop_web/new.ex", "ShopWeb.New");
        added.functions = vec![function(FunctionChangeKind::Added, "ShopWeb.New", "show")];
        let mut files = vec![added];

        let grouped = group_changed_files(&mut files, &matcher());

        assert_eq!(row(&grouped, "web").status, FileChangeStatus::Added);
    }
}
