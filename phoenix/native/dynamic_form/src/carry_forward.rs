//! `DynamicForm.CarryForward`: a question whose choices come from another
//! question, one per entry of a nested form or the other question's own
//! choices; and, for every choice question, dropping the values it
//! doesn't offer.

use std::collections::HashMap;

use ecto_changeset::Changeset;
use serde_json::{Map, Value};

use crate::definition::Question;
use crate::elixir;
use crate::nested::{ID_FIELD, list_entries};

const CHOICE_TYPES: [&str; 4] = ["dropdown", "radiogroup", "checkbox", "tagbox"];

/// What a carried-forward question reads when it renders: the questions
/// and current values of the form and of the entry it is in, if any.
#[derive(Debug, Clone, Default)]
pub struct Sources<'a> {
    pub questions: HashMap<&'a str, &'a Question>,
    pub entry_questions: HashMap<&'a str, &'a Question>,
    pub form_data: Map<String, Value>,
    pub entry_data: Map<String, Value>,
}

/// `CarryForward.resolve_choices/2`: the question's choices as `{label,
/// value}` pairs, its own or carried forward.
pub fn resolve_choices(question: &Question, sources: &Sources) -> Vec<(Value, Value)> {
    let Some(source_name) = &question.choices_from_question else {
        return question.own_choices();
    };
    let source = sources
        .entry_questions
        .get(source_name.as_str())
        .or_else(|| sources.questions.get(source_name.as_str()));

    match source {
        Some(source) if source.kind != "paneldynamic" => {
            carried_from_choices(source, question, sources)
        }
        _ => carried_from_entries(question, sources),
    }
}

fn carried_from_entries(question: &Question, sources: &Sources) -> Vec<(Value, Value)> {
    let name = question
        .choices_from_question
        .as_deref()
        .unwrap_or_default();
    let value = source_value(name, sources);
    if value.is_null() {
        return Vec::new();
    }
    list_entries(&value)
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            let entry = entry.as_object()?;
            let value_field = question
                .choice_values_from_question
                .as_deref()
                .unwrap_or(ID_FIELD);
            let value = entry.get(value_field).cloned().unwrap_or(Value::Null);
            let text = carried_text(entry, index, question);
            if blank(&value) || blank(&text) {
                None
            } else {
                Some((
                    Value::String(elixir::to_string(&text)),
                    Value::String(elixir::to_string(&value)),
                ))
            }
        })
        .collect()
}

fn carried_from_choices(
    source: &Question,
    question: &Question,
    sources: &Sources,
) -> Vec<(Value, Value)> {
    let choices = source.own_choices();
    match question.choices_from_question_mode.as_deref() {
        Some(mode @ ("selected" | "unselected")) => {
            let selected = texts(&source_value(&source.name, sources));
            choices
                .into_iter()
                .filter(|(_text, value)| {
                    selected.contains(&elixir::to_string(value)) == (mode == "selected")
                })
                .collect()
        }
        _ => choices,
    }
}

/// The entry's value first, then the form's, as `get_source_value/2`.
fn source_value(name: &str, sources: &Sources) -> Value {
    let entry = sources.entry_data.get(name).cloned().unwrap_or(Value::Null);
    if elixir::truthy(&entry) {
        entry
    } else {
        sources.form_data.get(name).cloned().unwrap_or(Value::Null)
    }
}

fn carried_text(entry: &Map<String, Value>, index: usize, question: &Question) -> Value {
    match &question.choice_texts_from_question {
        Some(template) if template.contains('{') => interpolate(template, entry, index),
        Some(field) => entry.get(field).cloned().unwrap_or(Value::Null),
        None => Value::Null,
    }
}

/// `"{min} - {max}"` against the entry's values, and `{panelIndex}`, the
/// 1-based position. A field without a value gives no text at all.
fn interpolate(template: &str, entry: &Map<String, Value>, index: usize) -> Value {
    let template = template.replace("{panelIndex}", &(index + 1).to_string());
    let mut fields = Vec::new();
    let mut rest = template.as_str();
    while let Some(start) = rest.find('{') {
        let after = &rest[start + 1..];
        match after.find('}') {
            Some(0) => rest = after,
            Some(end) => {
                fields.push(after[..end].to_string());
                rest = &after[end + 1..];
            }
            None => break,
        }
    }

    let values: Vec<String> = fields
        .iter()
        .map(|field| elixir::to_string(entry.get(field).unwrap_or(&Value::Null)))
        .collect();
    if values.iter().any(String::is_empty) {
        return Value::Null;
    }
    let text = fields
        .iter()
        .zip(&values)
        .fold(template.clone(), |text, (field, value)| {
            text.replace(&format!("{{{field}}}"), value)
        });
    Value::String(text.trim().to_string())
}

/// `CarryForward.blank?/1`: `nil` or `""`, unlike the definition's.
fn blank(value: &Value) -> bool {
    matches!(value, Value::Null) || value.as_str() == Some("")
}

fn texts(value: &Value) -> Vec<String> {
    match value {
        Value::Null => Vec::new(),
        Value::Array(items) => items.iter().map(elixir::to_string).collect(),
        other => vec![elixir::to_string(other)],
    }
}

/// `CarryForward.prune_values/4`: each choice question keeps only the
/// values it offers. `params` are the raw params a source is read from,
/// `scope` the questions a carried source is found among.
pub fn prune_values(
    mut changeset: Changeset,
    questions: &[&Question],
    scope: &[&Question],
    params: &Map<String, Value>,
) -> Changeset {
    for question in questions {
        if !CHOICE_TYPES.contains(&question.kind.as_str())
            && question.choices_from_question.is_none()
        {
            continue;
        }
        if let Some(allowed) = allowed_values(question, scope, params) {
            prune_field(&mut changeset, question, &allowed);
        }
    }
    changeset
}

/// The values the field may hold, or `None` when they can't be told.
fn allowed_values(
    question: &Question,
    scope: &[&Question],
    params: &Map<String, Value>,
) -> Option<Vec<String>> {
    let Some(source_name) = &question.choices_from_question else {
        return match &question.choices {
            None => None,
            Some(choices) if choices.is_empty() => None,
            Some(choices) => Some(choices.iter().map(|choice| choice.value_text()).collect()),
        };
    };
    let source = scope
        .iter()
        .find(|candidate| &candidate.name == source_name)?;

    if source.kind == "paneldynamic" {
        let value_field = question
            .choice_values_from_question
            .as_deref()
            .unwrap_or(ID_FIELD);
        let entries = list_entries(params.get(source_name).unwrap_or(&Value::Null));
        let values: Vec<String> = entries
            .iter()
            .filter_map(|entry| match entry.as_object()?.get(value_field)? {
                Value::Null => None,
                value => Some(elixir::to_string(value)),
            })
            .filter(|value| !value.is_empty())
            .collect();
        return if !params.contains_key(source_name) || (!entries.is_empty() && values.is_empty()) {
            None
        } else {
            Some(values)
        };
    }

    let values: Vec<String> = source
        .choices
        .iter()
        .flatten()
        .map(|choice| choice.value_text())
        .collect();
    match question.choices_from_question_mode.as_deref() {
        Some(mode @ ("selected" | "unselected")) => {
            if !params.contains_key(&source.name) {
                return None;
            }
            let selected = texts(&params[&source.name]);
            Some(
                values
                    .into_iter()
                    .filter(|value| selected.contains(value) == (mode == "selected"))
                    .collect(),
            )
        }
        _ => Some(values),
    }
}

fn prune_field(changeset: &mut Changeset, question: &Question, allowed: &[String]) {
    let Some(change) = changeset.changes().get(&question.name).cloned() else {
        return;
    };
    match change {
        Value::Array(values) => {
            let kept = values
                .into_iter()
                .filter(|value| allowed.contains(&elixir::to_string(value)))
                .collect();
            changeset.put_change(&question.name, Value::Array(kept));
        }
        value => {
            if !allowed.contains(&elixir::to_string(&value)) {
                changeset.delete_change(&question.name);
            }
        }
    }
}
