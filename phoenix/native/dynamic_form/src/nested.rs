//! `DynamicForm.NestedForms`: a `paneldynamic` question's entries, one
//! map per repetition of its template, and how entries are listed,
//! seeded, given ids, added and removed.

use serde_json::{Map, Value};

use crate::definition::{Element, Question, list_questions};

/// The entry field carrying a stable id, unless `generateIds` is `false`.
pub const ID_FIELD: &str = "dynamic_form_id";

/// `NestedForms.list_entries/1`: a list as it is; a browser's indexed map,
/// `{"0": ..., "1": ...}`, in the order of its integer keys, other keys
/// like `__empty__` left out; anything else none.
pub fn list_entries(value: &Value) -> Vec<Value> {
    match value {
        Value::Array(entries) => entries.clone(),
        Value::Object(indexed) => {
            let mut numbered: Vec<(i64, &Value)> = indexed
                .iter()
                .filter_map(|(key, entry)| integer_key(key).map(|index| (index, entry)))
                .collect();
            numbered.sort_by_key(|(index, _)| *index);
            numbered
                .into_iter()
                .map(|(_, entry)| entry.clone())
                .collect()
        }
        _ => Vec::new(),
    }
}

/// `Integer.parse/1` taking the whole key.
fn integer_key(key: &str) -> Option<i64> {
    let digits = key.strip_prefix(['+', '-']).unwrap_or(key);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let value: i64 = digits.parse().ok()?;
    Some(if key.starts_with('-') { -value } else { value })
}

/// The template an entry is validated against: the question's
/// `templateElements`, and its id field when it has one.
pub fn template_elements(question: &Question) -> Vec<Element> {
    let mut elements = question.template_elements.clone().unwrap_or_default();
    if question.generate_ids() {
        elements.push(Element::Question(Box::new(Question {
            name: ID_FIELD.to_string(),
            kind: "text".to_string(),
            ..Question::default()
        })));
    }
    elements
}

/// `NestedForms.new_entry/1`: the template questions' `defaultValue`s,
/// with the question's `defaultPanelValue` over them.
pub fn new_entry(question: &Question) -> Value {
    let template = question.template_elements.as_deref().unwrap_or_default();
    let mut entry = Map::new();
    for template_question in list_questions(template) {
        if let Some(default) = &template_question.default_value {
            entry.insert(template_question.name.clone(), default.clone());
        }
    }
    for (key, value) in question.default_panel_value.iter().flatten() {
        entry.insert(key.clone(), value.clone());
    }
    Value::Object(entry)
}

/// `NestedForms.seed_entries/2`: a nested form with no value gets
/// `panelCount` new entries, at least `minPanelCount`.
pub fn seed_entries(params: &mut Map<String, Value>, questions: &[&Question]) {
    for question in questions.iter().filter(|q| q.kind == "paneldynamic") {
        let count = question
            .panel_count
            .unwrap_or(0)
            .max(question.min_panel_count.unwrap_or(0))
            .max(0);
        let entries = (0..count).map(|_| new_entry(question)).collect();
        params
            .entry(question.name.clone())
            .or_insert(Value::Array(entries));
    }
}

/// `NestedForms.seed_entry_ids/2`: every entry gets an id once, its own
/// `id` when it came from a stored record, else a new one, recursing
/// into nested templates.
pub fn seed_entry_ids(params: &mut Map<String, Value>, questions: &[&Question]) {
    for question in questions.iter().filter(|q| q.kind == "paneldynamic") {
        let Some(value) = params.get(&question.name) else {
            continue;
        };
        if value.is_null() {
            continue;
        }
        let template = question.template_elements.as_deref().unwrap_or_default();
        let inner = list_questions(template);
        let entries = list_entries(value)
            .into_iter()
            .map(|entry| match entry {
                Value::Object(mut entry) => {
                    put_entry_id(&mut entry, question);
                    seed_entry_ids(&mut entry, &inner);
                    Value::Object(entry)
                }
                other => other,
            })
            .collect();
        params.insert(question.name.clone(), Value::Array(entries));
    }
}

fn put_entry_id(entry: &mut Map<String, Value>, question: &Question) {
    let blank = |value: Option<&Value>| match value {
        None | Some(Value::Null) => true,
        Some(value) => value.as_str() == Some(""),
    };
    if !question.generate_ids() || !blank(entry.get(ID_FIELD)) {
        return;
    }
    let id = if blank(entry.get("id")) {
        uuid::Uuid::new_v4().to_string()
    } else {
        crate::elixir::to_string(&entry["id"])
    };
    entry.insert(ID_FIELD.to_string(), Value::String(id));
}

/// The nested form question at a dot-separated path, `"addresses"` or
/// `"contacts.0.phones"`, as `Elements.get_question_by_path/2`.
pub fn find_question<'a>(elements: &'a [Element], path: &[&str]) -> Option<&'a Question> {
    let find = |elements: &'a [Element], name: &str| {
        list_questions(elements)
            .into_iter()
            .find(|q| q.kind == "paneldynamic" && q.name == name)
    };
    match path {
        [name] => find(elements, name),
        [name, _index, rest @ ..] => {
            let question = find(elements, name)?;
            find_question(question.template_elements.as_deref()?, rest)
        }
        [] => None,
    }
}

/// `NestedForms.update_entry_list/3`: walks the params along the path,
/// question names and entry indexes in turn, and replaces the entry list
/// at its end with `update` of it.
pub fn update_entry_list(
    params: &mut Value,
    path: &[&str],
    update: &dyn Fn(Vec<Value>) -> Vec<Value>,
) -> Result<(), String> {
    match (params, path) {
        (Value::Object(map), [name]) => {
            let entries = list_entries(map.get(*name).unwrap_or(&Value::Null));
            map.insert(name.to_string(), Value::Array(update(entries)));
            Ok(())
        }
        (Value::Object(map), [name, rest @ ..]) => {
            let mut inner = match map.get(*name) {
                None | Some(Value::Null) => Value::Object(Map::new()),
                Some(value) => value.clone(),
            };
            update_entry_list(&mut inner, rest, update)?;
            map.insert(name.to_string(), inner);
            Ok(())
        }
        (Value::Array(entries), [index, rest @ ..]) => {
            let index: usize = index
                .parse()
                .map_err(|_| format!("the entry index {index} is not a number"))?;
            // `List.update_at/3` leaves a list without the index alone
            match entries.get_mut(index) {
                Some(entry) => update_entry_list(entry, rest, update),
                None => Ok(()),
            }
        }
        (_, path) => Err(format!("no entry list at {}", path.join("."))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::definition::Definition;
    use serde_json::json;

    #[test]
    fn a_browsers_indexed_map_lists_in_key_order() {
        let entries = list_entries(&json!({"10": "c", "2": "b", "__empty__": "", "0": "a"}));
        assert_eq!(entries, [json!("a"), json!("b"), json!("c")]);
        assert!(list_entries(&json!("x")).is_empty());
    }

    #[test]
    fn new_entries_take_their_defaults_and_get_ids() {
        let definition = Definition::parse(
            r#"{"elements": [{"name": "rooms", "type": "paneldynamic", "panelCount": 1, "minPanelCount": 2,
                "defaultPanelValue": {"size": "big"},
                "templateElements": [{"name": "size", "type": "text", "defaultValue": "small"},
                                     {"name": "name", "type": "text", "defaultValue": "Room"}]}]}"#,
        )
        .unwrap();
        let questions = definition.questions();
        let mut params = Map::new();
        seed_entries(&mut params, &questions);
        assert_eq!(
            params["rooms"],
            json!([{"size": "big", "name": "Room"}, {"size": "big", "name": "Room"}])
        );

        params["rooms"][1]["id"] = json!(7);
        seed_entry_ids(&mut params, &questions);
        assert_eq!(params["rooms"][1][ID_FIELD], "7");
        assert_eq!(params["rooms"][0][ID_FIELD].as_str().unwrap().len(), 36);
    }

    #[test]
    fn an_entry_list_is_found_and_updated_along_its_path() {
        let mut params = json!({"contacts": [{"phones": [{"n": "1"}]}]});
        update_entry_list(&mut params, &["contacts", "0", "phones"], &|mut entries| {
            entries.push(json!({}));
            entries
        })
        .unwrap();
        assert_eq!(params, json!({"contacts": [{"phones": [{"n": "1"}, {}]}]}));
    }
}
