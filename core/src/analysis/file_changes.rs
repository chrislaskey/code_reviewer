use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};

use super::function_changes::{FunctionChange, FunctionChangeKind};
use super::model::FunctionId;
use super::module_rules::ModuleRules;
use super::project_index::CallLink;

/// What happened to a file, judged from its changed functions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileChangeStatus {
    /// Every changed function was added.
    Added,
    /// Anything else: edits, renames, or a mix of added and removed.
    Modified,
    /// Every changed function was removed.
    Removed,
}

/// One changed function, as listed under its file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileFunctionChange {
    pub kind: FunctionChangeKind,
    pub function: FunctionId,
    /// The name in the base. Only set when renamed.
    pub previous_function: Option<FunctionId>,
    /// Lines added or edited. Unknown for removed and renamed functions.
    pub added_line_count: Option<usize>,
    /// Lines deleted. Only known for removed functions, where it is the whole function.
    pub removed_line_count: Option<usize>,
}

/// A function in another module that calls one of the file's changed functions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileCaller {
    pub function: FunctionId,
    pub file_path: String,
    /// First line of the call.
    pub line: usize,
    /// The changed function in this file that is called.
    pub calls: FunctionId,
    /// Still calls a function that was removed or renamed.
    pub is_stale: bool,
    /// The caller is itself changed in this review.
    pub is_also_changed: bool,
}

/// A function in another module that one of the file's changed functions calls.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileCallee {
    pub function: FunctionId,
    /// False for the standard library and dependencies.
    pub defined_in_project: bool,
    /// Defined in the project, or matched by an include rule.
    pub is_highlighted: bool,
    /// The changed function in this file that makes the call.
    pub from: FunctionId,
    pub call_count: usize,
    /// The called function is itself changed in this review.
    pub is_also_changed: bool,
    /// The file that defines the function. Only known for functions the project defines.
    #[serde(default)]
    pub file_path: Option<String>,
}

/// Everything the review screen draws for one changed file.
///
/// Callers and callees follow the order of the changed functions. Within one
/// function, stale callers come first and calls into the project come before
/// calls out of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileChange {
    pub file_path: String,
    /// The module of the first changed function.
    pub module: String,
    pub status: FileChangeStatus,
    pub functions: Vec<FileFunctionChange>,
    pub callers: Vec<FileCaller>,
    pub callees: Vec<FileCallee>,
    /// Every group that holds the file. Empty when no groups are defined.
    #[serde(default)]
    pub group_ids: Vec<String>,
}

/// Group function changes by file, sorted by file path.
///
/// Callers and calls that match `rules.ignore` are left out, and calls that
/// match `rules.include` are highlighted like project calls.
pub fn group_changes_by_file(changes: &[FunctionChange], rules: &ModuleRules) -> Vec<FileChange> {
    let changed_ids: HashSet<&FunctionId> = changes.iter().map(|change| &change.function).collect();

    let mut changes_by_file: BTreeMap<&str, Vec<&FunctionChange>> = BTreeMap::new();
    for change in changes {
        changes_by_file
            .entry(change.file_path.as_str())
            .or_default()
            .push(change);
    }

    changes_by_file
        .into_iter()
        .map(|(file_path, file_changes)| {
            build_file_change(file_path, &file_changes, &changed_ids, rules)
        })
        .collect()
}

fn build_file_change(
    file_path: &str,
    changes: &[&FunctionChange],
    changed_ids: &HashSet<&FunctionId>,
    rules: &ModuleRules,
) -> FileChange {
    FileChange {
        file_path: file_path.to_string(),
        module: changes
            .first()
            .map(|change| change.function.module.clone())
            .unwrap_or_default(),
        status: file_status(changes),
        functions: changes
            .iter()
            .map(|change| build_function(change))
            .collect(),
        callers: changes
            .iter()
            .flat_map(|change| list_callers(change, changed_ids, rules))
            .collect(),
        callees: changes
            .iter()
            .flat_map(|change| list_callees(change, changed_ids, rules))
            .collect(),
        group_ids: Vec::new(),
    }
}

fn file_status(changes: &[&FunctionChange]) -> FileChangeStatus {
    let all = |kind: FunctionChangeKind| changes.iter().all(|change| change.kind == kind);
    if all(FunctionChangeKind::Added) {
        FileChangeStatus::Added
    } else if all(FunctionChangeKind::Removed) {
        FileChangeStatus::Removed
    } else {
        FileChangeStatus::Modified
    }
}

fn build_function(change: &FunctionChange) -> FileFunctionChange {
    let (added_line_count, removed_line_count) = match change.kind {
        FunctionChangeKind::Added | FunctionChangeKind::Modified => {
            (Some(change.changed_lines.len()), None)
        }
        FunctionChangeKind::Removed => (
            None,
            Some(change.end_line.saturating_sub(change.start_line) + 1),
        ),
        FunctionChangeKind::Renamed => (None, None),
    };
    FileFunctionChange {
        kind: change.kind,
        function: change.function.clone(),
        previous_function: change.previous_function.clone(),
        added_line_count,
        removed_line_count,
    }
}

fn build_caller(
    link: &CallLink,
    change: &FunctionChange,
    is_stale: bool,
    changed_ids: &HashSet<&FunctionId>,
) -> FileCaller {
    FileCaller {
        function: link.function.clone(),
        file_path: link.call_file_path.clone(),
        line: link.call_lines.first().copied().unwrap_or_default(),
        calls: change.function.clone(),
        is_stale,
        is_also_changed: changed_ids.contains(&link.function),
    }
}

/// Callers of one changed function, stale ones first.
///
/// A removed function lists its callers in the base, marking those that still
/// call it in the head. A renamed function lists the callers of its new name,
/// then the callers that still use the old name.
fn list_callers(
    change: &FunctionChange,
    changed_ids: &HashSet<&FunctionId>,
    rules: &ModuleRules,
) -> Vec<FileCaller> {
    let stale_ids: HashSet<&FunctionId> = change
        .stale_callers
        .iter()
        .map(|link| &link.function)
        .collect();

    let mut callers: Vec<FileCaller> = change
        .external_calls
        .calls_in
        .iter()
        .map(|link| {
            let is_stale =
                change.kind == FunctionChangeKind::Removed && stale_ids.contains(&link.function);
            build_caller(link, change, is_stale, changed_ids)
        })
        .collect();

    let already_listed: HashSet<&FunctionId> = change
        .external_calls
        .calls_in
        .iter()
        .map(|link| &link.function)
        .collect();
    for link in &change.stale_callers {
        let listed_as_stale =
            change.kind == FunctionChangeKind::Removed && already_listed.contains(&link.function);
        if !listed_as_stale {
            callers.push(build_caller(link, change, true, changed_ids));
        }
    }

    callers.retain(|caller| !rules.is_ignored(&caller.function));
    callers.sort_by(|a, b| (!a.is_stale, &a.function).cmp(&(!b.is_stale, &b.function)));
    callers
}

/// Calls out of one changed function, calls into the project first.
fn list_callees(
    change: &FunctionChange,
    changed_ids: &HashSet<&FunctionId>,
    rules: &ModuleRules,
) -> Vec<FileCallee> {
    let mut callees: Vec<FileCallee> = change
        .external_calls
        .calls_out
        .iter()
        .filter(|link| !rules.is_ignored(&link.function))
        .map(|link| FileCallee {
            function: link.function.clone(),
            defined_in_project: link.defined_in_project,
            is_highlighted: link.defined_in_project || rules.is_included(&link.function),
            from: change.function.clone(),
            call_count: link.call_lines.len().max(1),
            is_also_changed: changed_ids.contains(&link.function),
            file_path: None,
        })
        .collect();
    callees.sort_by(|a, b| (!a.is_highlighted, &a.function).cmp(&(!b.is_highlighted, &b.function)));
    callees
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::external_calls::ExternalCalls;

    fn group(changes: &[FunctionChange]) -> Vec<FileChange> {
        group_changes_by_file(changes, &ModuleRules::default())
    }

    fn id(module: &str, name: &str, arity: usize) -> FunctionId {
        FunctionId::new(module, name, arity)
    }

    fn link(function: FunctionId, in_project: bool, lines: &[usize]) -> CallLink {
        CallLink {
            function,
            defined_in_project: in_project,
            call_file_path: "lib/caller.ex".to_string(),
            call_lines: lines.to_vec(),
            crosses_module_boundary: true,
        }
    }

    fn change(kind: FunctionChangeKind, function: FunctionId, file_path: &str) -> FunctionChange {
        FunctionChange {
            kind,
            external_calls: ExternalCalls {
                function: function.clone(),
                calls_in: Vec::new(),
                calls_out: Vec::new(),
            },
            function,
            previous_function: None,
            file_path: file_path.to_string(),
            previous_file_path: None,
            start_line: 10,
            end_line: 14,
            changed_lines: vec![11, 12],
            previous_calls_in: Vec::new(),
            stale_callers: Vec::new(),
        }
    }

    #[test]
    fn test_changes_are_grouped_by_file_and_sorted_by_path() {
        let changes = vec![
            change(FunctionChangeKind::Modified, id("B", "one", 0), "lib/b.ex"),
            change(FunctionChangeKind::Modified, id("A", "one", 0), "lib/a.ex"),
            change(FunctionChangeKind::Modified, id("B", "two", 1), "lib/b.ex"),
        ];
        let files = group(&changes);

        let paths: Vec<_> = files.iter().map(|file| file.file_path.as_str()).collect();
        assert_eq!(paths, vec!["lib/a.ex", "lib/b.ex"]);
        assert_eq!(files[1].module, "B");
        assert_eq!(files[1].functions.len(), 2);
    }

    #[test]
    fn test_status_follows_the_changed_functions() {
        let status_of = |kinds: &[FunctionChangeKind]| {
            let changes: Vec<_> = kinds
                .iter()
                .enumerate()
                .map(|(index, kind)| change(*kind, id("A", "f", index), "lib/a.ex"))
                .collect();
            group(&changes)[0].status
        };
        use FunctionChangeKind::*;

        assert_eq!(status_of(&[Added, Added]), FileChangeStatus::Added);
        assert_eq!(status_of(&[Removed, Removed]), FileChangeStatus::Removed);
        assert_eq!(status_of(&[Modified]), FileChangeStatus::Modified);
        assert_eq!(status_of(&[Added, Removed]), FileChangeStatus::Modified);
        assert_eq!(status_of(&[Added, Modified]), FileChangeStatus::Modified);
        assert_eq!(status_of(&[Renamed]), FileChangeStatus::Modified);
    }

    #[test]
    fn test_line_counts_are_only_reported_where_known() {
        let counts = |kind| {
            let files = group(&[change(kind, id("A", "f", 0), "lib/a.ex")]);
            (
                files[0].functions[0].added_line_count,
                files[0].functions[0].removed_line_count,
            )
        };

        assert_eq!(counts(FunctionChangeKind::Added), (Some(2), None));
        assert_eq!(counts(FunctionChangeKind::Modified), (Some(2), None));
        assert_eq!(counts(FunctionChangeKind::Removed), (None, Some(5)));
        assert_eq!(counts(FunctionChangeKind::Renamed), (None, None));
    }

    #[test]
    fn test_callers_follow_function_order_and_flag_callers_that_also_changed() {
        let mut first = change(
            FunctionChangeKind::Modified,
            id("A", "first", 0),
            "lib/a.ex",
        );
        first.external_calls.calls_in = vec![
            link(id("Z", "zed", 0), true, &[3, 9]),
            link(id("B", "other", 1), true, &[5]),
        ];
        let mut second = change(
            FunctionChangeKind::Modified,
            id("A", "second", 0),
            "lib/a.ex",
        );
        second.external_calls.calls_in = vec![link(id("C", "third", 0), true, &[7])];
        let other_file = change(FunctionChangeKind::Added, id("B", "other", 1), "lib/b.ex");

        let files = group(&[first, second, other_file]);
        let callers = &files[0].callers;

        let order: Vec<_> = callers
            .iter()
            .map(|caller| caller.function.to_string())
            .collect();
        assert_eq!(order, vec!["B.other/1", "Z.zed/0", "C.third/0"]);
        assert_eq!(callers[0].calls, id("A", "first", 0));
        assert_eq!(callers[1].line, 3);
        assert!(callers[0].is_also_changed);
        assert!(!callers[1].is_also_changed);
    }

    #[test]
    fn test_removed_function_marks_the_callers_that_still_call_it() {
        let mut removed = change(FunctionChangeKind::Removed, id("A", "gone", 0), "lib/a.ex");
        removed.external_calls.calls_in = vec![
            link(id("B", "fixed", 0), true, &[1]),
            link(id("C", "broken", 0), true, &[2]),
        ];
        removed.stale_callers = vec![link(id("C", "broken", 0), true, &[2])];

        let files = group(&[removed]);
        let callers = &files[0].callers;

        assert_eq!(callers.len(), 2);
        assert_eq!(callers[0].function, id("C", "broken", 0));
        assert!(callers[0].is_stale);
        assert!(!callers[1].is_stale);
    }

    #[test]
    fn test_renamed_function_adds_callers_of_the_old_name_as_stale() {
        let mut renamed = change(FunctionChangeKind::Renamed, id("A", "new", 0), "lib/a.ex");
        renamed.previous_function = Some(id("A", "old", 0));
        renamed.external_calls.calls_in = vec![link(id("B", "updated", 0), true, &[1])];
        renamed.stale_callers = vec![link(id("C", "behind", 0), true, &[2])];

        let files = group(&[renamed]);
        let callers = &files[0].callers;

        assert_eq!(callers.len(), 2);
        assert_eq!(callers[0].function, id("C", "behind", 0));
        assert!(callers[0].is_stale);
        assert_eq!(callers[0].calls, id("A", "new", 0));
        assert!(!callers[1].is_stale);
    }

    #[test]
    fn test_callees_list_project_calls_first_and_count_call_lines() {
        let mut modified = change(FunctionChangeKind::Modified, id("A", "run", 0), "lib/a.ex");
        modified.external_calls.calls_out = vec![
            link(id("Enum", "map", 2), false, &[4, 5]),
            link(id("B", "work", 1), true, &[6]),
        ];

        let files = group(&[modified]);
        let callees = &files[0].callees;

        assert_eq!(callees[0].function, id("B", "work", 1));
        assert!(callees[0].defined_in_project);
        assert_eq!(callees[1].function, id("Enum", "map", 2));
        assert_eq!(callees[1].call_count, 2);
        assert_eq!(callees[1].from, id("A", "run", 0));
    }

    #[test]
    fn test_ignore_rules_drop_callers_and_calls() {
        let mut modified = change(FunctionChangeKind::Modified, id("A", "run", 0), "lib/a.ex");
        modified.external_calls.calls_in = vec![
            link(id("ATest", "checks", 0), true, &[1]),
            link(id("B", "caller", 0), true, &[2]),
        ];
        modified.external_calls.calls_out = vec![
            link(id("Enum", "reduce", 3), false, &[3]),
            link(id("Enum", "map", 2), false, &[4]),
        ];
        let rules = ModuleRules {
            include: Vec::new(),
            ignore: vec!["ATest".to_string(), "Enum.reduce".to_string()],
        };

        let files = group_changes_by_file(&[modified], &rules);

        assert_eq!(files[0].callers.len(), 1);
        assert_eq!(files[0].callers[0].function, id("B", "caller", 0));
        assert_eq!(files[0].callees.len(), 1);
        assert_eq!(files[0].callees[0].function, id("Enum", "map", 2));
    }

    #[test]
    fn test_include_rules_highlight_calls_the_project_does_not_define() {
        let mut modified = change(FunctionChangeKind::Modified, id("A", "run", 0), "lib/a.ex");
        modified.external_calls.calls_out = vec![
            link(id("Enum", "map", 2), false, &[1]),
            link(id("Ecto.Repo", "insert", 1), false, &[2]),
            link(id("B", "work", 0), true, &[3]),
        ];
        let rules = ModuleRules {
            include: vec!["Ecto.Repo".to_string()],
            ignore: vec!["Ecto.Repo".to_string(), "Enum*".to_string()],
        };
        let only_include = ModuleRules {
            include: rules.include.clone(),
            ignore: Vec::new(),
        };

        let callees = &group_changes_by_file(&[modified.clone()], &only_include)[0].callees;
        let highlighted: Vec<_> = callees
            .iter()
            .map(|callee| (callee.function.to_string(), callee.is_highlighted))
            .collect();
        assert_eq!(
            highlighted,
            vec![
                ("B.work/0".to_string(), true),
                ("Ecto.Repo.insert/1".to_string(), true),
                ("Enum.map/2".to_string(), false),
            ]
        );

        // Ignoring wins over including.
        let callees = &group_changes_by_file(&[modified], &rules)[0].callees;
        assert_eq!(callees.len(), 1);
        assert_eq!(callees[0].function, id("B", "work", 0));
    }
}
