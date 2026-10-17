use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};

use super::changed_functions::{list_changed_functions, ChangedFunction};
use super::error::AnalysisError;
use super::external_calls::{list_external_calls_for_function, ExternalCalls};
use super::function_source::extract_function_source;
use super::model::{FunctionDefinition, FunctionId};
use super::project_index::{CallLink, ProjectIndex};
use crate::git::ChangedFileLines;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FunctionChangeKind {
    Added,
    Modified,
    /// In the base but not in the head, and not matched to an added function.
    Removed,
    /// Same body under a new name or in a new module.
    Renamed,
}

/// One function that was added, modified, removed or renamed between two snapshots.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionChange {
    pub kind: FunctionChangeKind,
    /// The function in the head, or in the base when it was removed.
    pub function: FunctionId,
    /// The name in the base. Only set when renamed.
    pub previous_function: Option<FunctionId>,
    pub file_path: String,
    pub previous_file_path: Option<String>,
    pub start_line: usize,
    pub end_line: usize,
    /// Changed lines in the head. Empty when removed.
    pub changed_lines: Vec<usize>,
    /// One-level calls across modules. From the base when removed, otherwise from the head.
    pub external_calls: ExternalCalls,
    /// Other-module callers of the old name in the base. Only set when renamed.
    pub previous_calls_in: Vec<CallLink>,
    /// Other-module callers in the head that still call the old name. Set when removed or renamed.
    pub stale_callers: Vec<CallLink>,
}

/// Compare two snapshots of a project function by function.
///
/// `changed_files` are the changed line ranges in the head, used to find
/// which functions in both snapshots were touched.
pub fn list_function_changes(
    base_index: &ProjectIndex,
    head_index: &ProjectIndex,
    changed_files: &[ChangedFileLines],
) -> Result<Vec<FunctionChange>, AnalysisError> {
    let added: Vec<&FunctionDefinition> = head_index
        .list_functions()
        .filter(|function| base_index.get_function(&function.id).is_none())
        .collect();
    let removed: Vec<&FunctionDefinition> = base_index
        .list_functions()
        .filter(|function| head_index.get_function(&function.id).is_none())
        .collect();

    let mut texts = FunctionTexts::default();
    let renames = pair_renamed_functions(base_index, head_index, &removed, &added, &mut texts)?;
    let renamed_from: HashMap<&FunctionId, &FunctionDefinition> =
        renames.iter().map(|(old, new)| (&new.id, *old)).collect();
    let renamed_old_ids: Vec<&FunctionId> = renames.iter().map(|(old, _)| &old.id).collect();

    let mut changes = Vec::new();

    for head_function in &added {
        let change = match renamed_from.get(&head_function.id) {
            Some(old) => build_renamed(base_index, head_index, old, head_function)?,
            None => build_added(head_index, head_function)?,
        };
        changes.push(change);
    }

    for base_function in &removed {
        if !renamed_old_ids.contains(&&base_function.id) {
            changes.push(build_removed(base_index, head_index, base_function)?);
        }
    }

    for touched in list_changed_functions(head_index, changed_files) {
        let Some(base_function) = base_index.get_function(&touched.function) else {
            continue;
        };
        let Some(head_function) = head_index.get_function(&touched.function) else {
            continue;
        };
        // Same text means the function only moved with its file.
        let base_text = texts.get(base_index, base_function)?;
        let head_text = texts.get(head_index, head_function)?;
        if base_text != head_text {
            changes.push(build_modified(head_index, &touched)?);
        }
    }

    changes.sort_by(|a, b| {
        (&a.file_path, a.start_line, &a.function).cmp(&(&b.file_path, b.start_line, &b.function))
    });
    Ok(changes)
}

/// Pairs a removed function with an added one when their bodies match.
/// Only unambiguous one-to-one matches count.
fn pair_renamed_functions<'a>(
    base_index: &ProjectIndex,
    head_index: &ProjectIndex,
    removed: &[&'a FunctionDefinition],
    added: &[&'a FunctionDefinition],
    texts: &mut FunctionTexts,
) -> Result<Vec<(&'a FunctionDefinition, &'a FunctionDefinition)>, AnalysisError> {
    if removed.is_empty() || added.is_empty() {
        return Ok(Vec::new());
    }

    let mut removed_by_body: BTreeMap<String, Vec<&FunctionDefinition>> = BTreeMap::new();
    for function in removed {
        let body = texts.get_without_name(base_index, function)?;
        removed_by_body.entry(body).or_default().push(function);
    }
    let mut added_by_body: BTreeMap<String, Vec<&FunctionDefinition>> = BTreeMap::new();
    for function in added {
        let body = texts.get_without_name(head_index, function)?;
        added_by_body.entry(body).or_default().push(function);
    }

    let mut pairs = Vec::new();
    for (body, removed_matches) in removed_by_body {
        if let (&[old], Some(&[new])) = (
            removed_matches.as_slice(),
            added_by_body.get(&body).map(Vec::as_slice),
        ) {
            pairs.push((old, new));
        }
    }
    Ok(pairs)
}

fn whole_function_lines(function: &FunctionDefinition) -> Vec<usize> {
    (function.start_line..=function.end_line).collect()
}

fn external_only(links: Vec<CallLink>) -> Vec<CallLink> {
    links
        .into_iter()
        .filter(|link| link.crosses_module_boundary)
        .collect()
}

fn build_added(
    head_index: &ProjectIndex,
    function: &FunctionDefinition,
) -> Result<FunctionChange, AnalysisError> {
    Ok(FunctionChange {
        kind: FunctionChangeKind::Added,
        function: function.id.clone(),
        previous_function: None,
        file_path: function.file_path.clone(),
        previous_file_path: None,
        start_line: function.start_line,
        end_line: function.end_line,
        changed_lines: whole_function_lines(function),
        external_calls: list_external_calls_for_function(head_index, &function.id)?,
        previous_calls_in: Vec::new(),
        stale_callers: Vec::new(),
    })
}

fn build_modified(
    head_index: &ProjectIndex,
    touched: &ChangedFunction,
) -> Result<FunctionChange, AnalysisError> {
    Ok(FunctionChange {
        kind: FunctionChangeKind::Modified,
        function: touched.function.clone(),
        previous_function: None,
        file_path: touched.file_path.clone(),
        previous_file_path: None,
        start_line: touched.start_line,
        end_line: touched.end_line,
        changed_lines: touched.changed_lines.clone(),
        external_calls: list_external_calls_for_function(head_index, &touched.function)?,
        previous_calls_in: Vec::new(),
        stale_callers: Vec::new(),
    })
}

fn build_renamed(
    base_index: &ProjectIndex,
    head_index: &ProjectIndex,
    old: &FunctionDefinition,
    new: &FunctionDefinition,
) -> Result<FunctionChange, AnalysisError> {
    Ok(FunctionChange {
        kind: FunctionChangeKind::Renamed,
        function: new.id.clone(),
        previous_function: Some(old.id.clone()),
        file_path: new.file_path.clone(),
        previous_file_path: Some(old.file_path.clone()),
        start_line: new.start_line,
        end_line: new.end_line,
        changed_lines: whole_function_lines(new),
        external_calls: list_external_calls_for_function(head_index, &new.id)?,
        previous_calls_in: external_only(base_index.list_incoming_call_links(&old.id)),
        stale_callers: external_only(head_index.list_incoming_call_links(&old.id)),
    })
}

fn build_removed(
    base_index: &ProjectIndex,
    head_index: &ProjectIndex,
    function: &FunctionDefinition,
) -> Result<FunctionChange, AnalysisError> {
    Ok(FunctionChange {
        kind: FunctionChangeKind::Removed,
        function: function.id.clone(),
        previous_function: None,
        file_path: function.file_path.clone(),
        previous_file_path: None,
        start_line: function.start_line,
        end_line: function.end_line,
        changed_lines: Vec::new(),
        external_calls: list_external_calls_for_function(base_index, &function.id)?,
        previous_calls_in: Vec::new(),
        stale_callers: external_only(head_index.list_incoming_call_links(&function.id)),
    })
}

/// Reads each file once and cuts function text out of it.
#[derive(Default)]
struct FunctionTexts {
    file_texts: HashMap<(String, String), String>,
}

impl FunctionTexts {
    /// Function text with trailing whitespace removed from each line.
    fn get(
        &mut self,
        index: &ProjectIndex,
        function: &FunctionDefinition,
    ) -> Result<String, AnalysisError> {
        let key = (format!("{:?}", index.origin()), function.file_path.clone());
        if !self.file_texts.contains_key(&key) {
            let text = index.read_file_text(&function.file_path)?;
            self.file_texts.insert(key.clone(), text);
        }
        let file_text = self.file_texts.get(&key).map_or("", String::as_str);

        Ok(extract_function_source(function, file_text)
            .lines
            .iter()
            .map(|line| line.text.trim_end())
            .collect::<Vec<_>>()
            .join("\n"))
    }

    /// Like `get`, with the function name blanked so renamed copies compare equal.
    fn get_without_name(
        &mut self,
        index: &ProjectIndex,
        function: &FunctionDefinition,
    ) -> Result<String, AnalysisError> {
        let text = self.get(index, function)?;
        Ok(text.replace(&function.id.name, "\u{0}"))
    }
}
