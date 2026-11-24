//! `cases.json` is what the `dynamic_form` library does, and this crate
//! must do the same: `test/app/web/components/dynamic_form_cases_test.exs`
//! in the Phoenix app holds the library to the same file.
//!
//! - `cases`: the errors a definition gives for params, a nested entry's
//!   under `name.index.field`, and the changes when a case lists them
//! - `expressions`: what a condition evaluates to over raw params
//! - `views`: what a form shows for params, as an outline of the rendered
//!   form, one line per control, error, heading, note and button. The
//!   Elixir test reads it from the library's HTML; this one builds it
//!   from the view, as `DynamicForm.svelte` and the components render it
//! - `flows`: the form's lifecycle, from data through changes, entries
//!   added and removed, and submits, with the outline after each step and
//!   a submit's result. An entry's generated id reads as `<id>`

use dynamic_form::expression::{Keys, Scope, evaluate};
use dynamic_form::view::{ElementView, EntryView, NestedView};
use dynamic_form::{Definition, DynamicForm, FormTarget, FormView, changeset, view};
use serde_json::{Value, json};

fn file() -> Value {
    serde_json::from_str(include_str!("cases.json")).expect("cases.json")
}

fn definition(file: &Value, name: &str) -> Definition {
    Definition::from_value(&file["definitions"][name])
        .unwrap_or_else(|error| panic!("{name}: {error}"))
}

#[test]
fn the_errors_are_the_librarys() {
    let file = file();
    let mut differences = Vec::new();

    for case in file["cases"].as_array().expect("cases") {
        let definition = definition(&file, case["definition"].as_str().unwrap());
        let params = case["params"].as_object().cloned().expect("params");

        let changeset = changeset(&definition.elements, &params);
        let mut errors: Vec<Value> = changeset
            .all_errors()
            .iter()
            .map(|error| {
                let options: Vec<Value> = error
                    .options
                    .iter()
                    .map(|(key, value)| json!([key, value]))
                    .collect();
                json!([error.field, error.message, options])
            })
            .collect();
        errors.sort_by_key(Value::to_string);

        let mut expected = case["errors"].as_array().cloned().expect("errors");
        expected.sort_by_key(Value::to_string);

        if errors != expected {
            differences.push(format!(
                "{}:\n  cases.json: {}\n  rust:       {}",
                case["name"],
                Value::Array(expected),
                Value::Array(errors)
            ));
        }

        if let Some(expected) = case.get("changes") {
            let changes = Value::Object(changeset.data());
            if &changes != expected {
                differences.push(format!(
                    "{} changes:\n  cases.json: {expected}\n  rust:       {changes}",
                    case["name"]
                ));
            }
        }
    }

    assert!(differences.is_empty(), "{}", differences.join("\n"));
}

#[test]
fn the_conditions_are_the_librarys() {
    let file = file();
    let mut differences = Vec::new();

    for case in file["expressions"].as_array().expect("expressions") {
        let expression = case["expression"].as_str().unwrap();
        let params = case["params"].as_object().cloned().unwrap_or_default();
        let result = evaluate(expression, Scope::new(&params, Keys::Strings));
        if Value::Bool(result) != case["expected"] {
            differences.push(format!(
                "{expression} over {}: rust says {result}",
                case["params"]
            ));
        }
    }

    assert!(differences.is_empty(), "{}", differences.join("\n"));
}

#[test]
fn the_views_are_the_librarys() {
    let file = file();
    let mut differences = Vec::new();

    for case in file["views"].as_array().expect("views") {
        let definition = definition(&file, case["definition"].as_str().unwrap());
        let params = case["params"].as_object().cloned().unwrap_or_default();
        let form = changeset(&definition.elements, &params);
        let target = FormTarget {
            id: "f",
            name: "f",
            submitted: case["submitted"].as_bool().unwrap_or(false),
        };
        let shown = view(&definition, target, &form);

        let outline = outline(&shown);

        let expected: Vec<String> = case["outline"]
            .as_array()
            .expect("outline")
            .iter()
            .map(|line| line.as_str().unwrap().to_string())
            .collect();
        if outline != expected {
            differences.push(format!(
                "{}:\n  cases.json: {expected:#?}\n  rust:       {outline:#?}",
                case["name"]
            ));
        }
    }

    assert!(differences.is_empty(), "{}", differences.join("\n"));
}

#[test]
fn the_flows_are_the_librarys() {
    let file = file();
    let mut differences = Vec::new();

    for case in file["flows"].as_array().expect("flows") {
        let definition = definition(&file, case["definition"].as_str().unwrap());
        let data = case["data"].as_object().cloned().unwrap_or_default();
        let form = DynamicForm::new(&definition, "f-form", "f").with_data(data);

        let mut shown = form.mount();
        let mut outlines = vec![outline(&shown.view)];
        for step in case["steps"].as_array().expect("steps") {
            // As LiveViewTest sends a form event: the form's values, as a
            // browser's `FormData` has them, with the step's over them
            let view = shown.view.clone();
            let params = |key: &str| {
                let mut defaults = form_data(&view);
                deep_merge(&mut defaults, &step[key]);
                defaults.as_object().cloned().unwrap_or_default()
            };
            if step.get("change").is_some() {
                shown = form.change_params(&shown.state, &params("change"));
                outlines.push(outline(&shown.view));
            } else if let Some(path) = step["add"].as_str() {
                shown = form.add_entry(&shown.state, path).expect("an entry added");
                outlines.push(outline(&shown.view));
            } else if let Some([path, index]) = step["remove"].as_array().map(Vec::as_slice) {
                let index = index.as_u64().unwrap() as usize;
                shown = form
                    .remove_entry(&shown.state, path.as_str().unwrap(), index)
                    .expect("an entry removed");
                outlines.push(outline(&shown.view));
            } else {
                // A valid submit leaves the form as it was, as the
                // library's component does, and hands over the data
                let result = match form.submit_params(&shown.state, &params("submit")) {
                    Ok(data) => ids(&Value::Object(data).to_string()),
                    Err(invalid) => {
                        shown = invalid;
                        "invalid".to_string()
                    }
                };
                let mut lines = outline(&shown.view);
                lines.push(format!("result {result}"));
                outlines.push(lines);
            }
        }

        let expected: Vec<Vec<String>> =
            serde_json::from_value(case["outlines"].clone()).expect("outlines");
        if !same_outlines(&outlines, &expected) {
            differences.push(format!(
                "{}:\n  cases.json: {expected:#?}\n  rust:       {outlines:#?}",
                case["name"]
            ));
        }
    }

    assert!(differences.is_empty(), "{}", differences.join("\n"));
}

/// The form's params as a browser's `FormData` has them, decoded: no
/// disabled control, a checkbox or radio only when checked, and a single
/// select's first option when none is selected.
fn form_data(view: &FormView) -> Value {
    let mut entries = Vec::new();
    data_entries(&view.elements, &mut entries);
    let mut params =
        ecto_changeset::form::decode(entries.iter().map(|(n, v)| (n.as_str(), v.as_str())))
            .expect("form data");
    params.remove("f").unwrap_or(json!({}))
}

fn data_entries(views: &[ElementView], entries: &mut Vec<(String, String)>) {
    for element in views {
        match element {
            ElementView::Field {
                component, props, ..
            } => {
                if props["disabled"] == true {
                    continue;
                }
                let name = text(&props["name"]);
                let options = props["options"].as_array().cloned().unwrap_or_default();
                let marked = |key: &str| -> Vec<String> {
                    options
                        .iter()
                        .filter(|o| o[key] == true)
                        .map(|o| text(&o["value"]))
                        .collect()
                };
                match *component {
                    "TextInput" | "TextareaInput" => entries.push((name, text(&props["value"]))),
                    "SelectInput" if props["multiple"] == true => {
                        entries.extend(marked("selected").into_iter().map(|v| (name.clone(), v)))
                    }
                    "SelectInput" => {
                        let first = if props["prompt"].is_string() {
                            Some(String::new())
                        } else {
                            options.first().map(|o| text(&o["value"]))
                        };
                        if let Some(value) = marked("selected").pop().or(first) {
                            entries.push((name, value));
                        }
                    }
                    "CheckboxInput" => {
                        entries.push((name.clone(), "false".into()));
                        if props["checked"] == true {
                            entries.push((name, "true".into()));
                        }
                    }
                    "RadioGroupInput" => {
                        entries.extend(marked("checked").into_iter().map(|v| (name.clone(), v)))
                    }
                    "CheckboxGroupInput" => {
                        entries.push((name.clone(), String::new()));
                        entries.extend(marked("checked").into_iter().map(|v| (name.clone(), v)));
                    }
                    _ => {}
                }
            }
            ElementView::Hidden { inputs } => {
                entries.extend(inputs.iter().map(|i| (i.name.clone(), i.value.clone())))
            }
            ElementView::Group { elements, .. } => data_entries(elements, entries),
            ElementView::Nested(nested) => {
                entries.push((nested.empty_input.clone(), String::new()));
                for entry in &nested.entries {
                    if let Some(input) = &entry.id_input {
                        entries.push((input.name.clone(), input.value.clone()));
                    }
                    data_entries(&entry.elements, entries);
                }
            }
            _ => {}
        }
    }
}

/// LiveViewTest's `deep_merge/2`.
fn deep_merge(target: &mut Value, source: &Value) {
    match (target, source) {
        (Value::Object(target), Value::Object(source)) => {
            for (key, value) in source {
                match target.get_mut(key) {
                    Some(existing) => deep_merge(existing, value),
                    None => {
                        target.insert(key.clone(), value.clone());
                    }
                }
            }
        }
        (target, source) => *target = source.clone(),
    }
}

/// The outlines are the same, a result's data compared as JSON, whose
/// key order the library doesn't fix.
fn same_outlines(actual: &[Vec<String>], expected: &[Vec<String>]) -> bool {
    let json = |line: &str| -> Value {
        match line.strip_prefix("result ") {
            Some(data) => serde_json::from_str(data).unwrap_or_else(|_| Value::from(data)),
            None => Value::from(line),
        }
    };
    actual.len() == expected.len()
        && actual.iter().zip(expected).all(|(actual, expected)| {
            actual.len() == expected.len()
                && actual.iter().zip(expected).all(|(a, e)| json(a) == json(e))
        })
}

fn outline(view: &FormView) -> Vec<String> {
    let mut outline = Vec::new();
    elements(&view.elements, &mut outline);
    outline.push("button Submit".to_string());
    outline.iter().map(|line| ids(line)).collect()
}

fn ids(text: &str) -> String {
    let uuid =
        regex::Regex::new(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}").unwrap();
    uuid.replace_all(text, "<id>").into_owned()
}

fn elements(views: &[ElementView], outline: &mut Vec<String>) {
    for element in views {
        match element {
            ElementView::Field {
                component,
                props,
                description,
                ..
            } => {
                field(component, props, outline);
                if let Some(description) = description {
                    outline.push(format!("p {description}"));
                }
            }
            ElementView::Hidden { inputs } => {
                for input in inputs {
                    outline.push(format!("input hidden {}={}", input.name, input.value));
                }
            }
            ElementView::NoChoices { text, .. } => outline.push(format!("p {text}")),
            ElementView::Html { html } => html_lines(html, outline),
            ElementView::Image { .. } => {}
            ElementView::Group {
                title,
                elements: members,
                ..
            } => {
                if let Some(title) = title {
                    outline.push(format!("h3 {title}"));
                }
                elements(members, outline);
            }
            ElementView::Nested(nested_view) => nested(nested_view, outline),
        }
    }
}

/// What each field component renders, as the outline sees it.
fn field(component: &str, props: &Value, outline: &mut Vec<String>) {
    let name = text(&props["name"]);
    let flags = |props: &Value, keys: &[&str]| -> String {
        keys.iter()
            .filter(|key| props[**key] == true)
            .map(|key| format!(" {key}"))
            .collect()
    };
    match component {
        "TextInput" => outline.push(format!(
            "input {} {name}={}{}",
            text(&props["type"]),
            text(&props["value"]),
            flags(props, &["disabled", "readonly"])
        )),
        "TextareaInput" => outline.push(format!(
            "textarea {name}={}{}",
            text(&props["value"]),
            flags(props, &["disabled", "readonly"])
        )),
        "SelectInput" => {
            let selected: Vec<String> = props["options"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|option| option["selected"] == true)
                .map(|option| text(&option["value"]))
                .collect();
            outline.push(format!(
                "select {name} [{}]{}",
                selected.join(","),
                flags(props, &["disabled"])
            ));
        }
        "CheckboxInput" => {
            outline.push(format!(
                "input hidden {name}=false{}",
                flags(props, &["disabled"])
            ));
            outline.push(format!(
                "input checkbox {name}=true{}",
                flags(props, &["checked", "disabled"])
            ));
        }
        "RadioGroupInput" | "CheckboxGroupInput" => {
            let kind = if component == "RadioGroupInput" {
                "radio"
            } else {
                "checkbox"
            };
            if kind == "checkbox" {
                outline.push(format!(
                    "input hidden {name}={}",
                    flags(props, &["disabled"])
                ));
            }
            for option in props["options"].as_array().into_iter().flatten() {
                let checked = if option["checked"] == true {
                    " checked"
                } else {
                    ""
                };
                outline.push(format!(
                    "input {kind} {name}={}{checked}{}",
                    text(&option["value"]),
                    flags(props, &["disabled"])
                ));
            }
        }
        other => panic!("no outline for {other}"),
    }
    for error in props["errors"].as_array().into_iter().flatten() {
        outline.push(format!("error {}", text(error)));
    }
}

fn nested(nested: &NestedView, outline: &mut Vec<String>) {
    if let Some(label) = &nested.label {
        let mark = match (&nested.required_label, nested.required) {
            (Some(mark), true) => mark.as_str(),
            _ => "",
        };
        outline.push(format!("h3 {label}{mark}"));
    }
    if let Some(add) = &nested.add_text {
        outline.push(format!("button {add}"));
    }
    outline.push(format!("input hidden {}=", nested.empty_input));
    if let Some(text) = &nested.no_entries_text {
        outline.push(format!("p {text}"));
    }
    for entry in &nested.entries {
        entry_lines(entry, outline);
    }
    for error in &nested.errors {
        outline.push(format!("error {error}"));
    }
}

fn entry_lines(entry: &EntryView, outline: &mut Vec<String>) {
    if let Some(title) = &entry.title {
        outline.push(format!("h4 {title}"));
    }
    if let Some(input) = &entry.id_input {
        outline.push(format!("input hidden {}={}", input.name, input.value));
    }
    elements(&entry.elements, outline);
    if let Some(remove) = &entry.remove {
        outline.push(format!("button {}", remove.label));
    }
}

/// The headings and paragraphs of a definition's html, the only markup
/// the cases put in one.
fn html_lines(html: &str, outline: &mut Vec<String>) {
    let tags = regex::Regex::new(r"<(h2|h3|h4|p)[^>]*>([^<]*)</(?:h2|h3|h4|p)>").unwrap();
    for captures in tags.captures_iter(html) {
        outline.push(format!("{} {}", &captures[1], &captures[2]));
    }
}

fn text(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}
