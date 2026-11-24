//! What a form shows, as `DynamicForm.Renderer.Component` renders it: the
//! visible elements in order, each question as the field component the
//! Phoenix app's `CoreComponents.input/1` renders with that component's
//! props, and the markup the library writes itself (panels, html, images,
//! nested forms) as data. The Svelte `DynamicForm.svelte` renders this.
//!
//! The renderer reads conditions against the changeset's cast changes,
//! where validation reads the raw params, so the two can disagree, as in
//! the library: a number question's value is a `Decimal` there, which
//! compares to nothing.

use std::collections::HashSet;

use ecto_changeset::{FieldType, Params};
use serde::Serialize;
use serde_json::{Map, Value, json};

use crate::carry_forward::{Sources, resolve_choices};
use crate::changeset::{FormChangeset, entry_changesets, field_type};
use crate::definition::{
    Container, Definition, Element, Question, list_questions, questions_by_name,
};
use crate::elixir;
use crate::expression::{Keys, Scope, condition_met};
use crate::nested::ID_FIELD;

/// One rendered element.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ElementView {
    /// A question: the component and its props, wrapped in `div.mb-4`
    /// with the description below, unless the component shows it.
    Field {
        name: String,
        component: &'static str,
        props: Value,
        description: Option<String>,
    },
    /// The hidden inputs carrying a read-only question's value, which its
    /// disabled control doesn't submit.
    Hidden {
        inputs: Vec<HiddenInput>,
    },
    /// A carried-forward question with nothing to choose from yet.
    NoChoices {
        label: Option<String>,
        required: bool,
        required_label: Option<String>,
        text: String,
    },
    Html {
        html: String,
    },
    Image {
        src: String,
        alt: String,
        style: String,
    },
    /// A panel, `dynamic_form_group/1`.
    Group {
        group_type: String,
        name: String,
        title: Option<String>,
        disabled: bool,
        elements: Vec<ElementView>,
    },
    Nested(NestedView),
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HiddenInput {
    pub name: String,
    pub value: String,
}

/// A nested form: its heading, its entries, and the add and remove
/// buttons, which send `add_entry` and `remove_entry` with `path`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NestedView {
    pub name: String,
    pub path: String,
    pub label: Option<String>,
    pub required: bool,
    pub required_label: Option<String>,
    pub description: Option<String>,
    pub add_text: Option<String>,
    /// The hidden input keeping the field in the params with no entries.
    pub empty_input: String,
    pub no_entries_text: Option<String>,
    pub entries: Vec<EntryView>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EntryView {
    pub index: usize,
    pub title: Option<String>,
    pub id_input: Option<HiddenInput>,
    pub elements: Vec<ElementView>,
    pub remove: Option<RemoveButton>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RemoveButton {
    pub label: String,
    pub confirm: Option<String>,
}

/// The form as shown.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FormView {
    pub elements: Vec<ElementView>,
    pub valid: bool,
}

/// The `<form>` a view is for: the element's id and the name its params
/// are under, as in `post[body]`, and whether a submit has failed, which
/// is when the library starts showing errors.
#[derive(Debug, Clone, Copy)]
pub struct FormTarget<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub submitted: bool,
}

/// The view of `form`, the changeset built from the form's params.
pub fn view(definition: &Definition, target: FormTarget, form: &FormChangeset) -> FormView {
    let questions = definition.questions();
    let changes = form.data();
    let context = Context {
        id: target.id.to_string(),
        name: target.name.to_string(),
        submitted: target.submitted,
        params: form.changeset.params().clone(),
        changes: changes.clone(),
        conditions: condition_values(&changes, &questions),
        errors: errors_by_field(form),
    };
    let options = Options {
        disabled: false,
        path: Vec::new(),
        sources: Sources {
            questions: questions_by_name(&definition.elements),
            form_data: changes,
            ..Sources::default()
        },
    };
    let scope = Scope::new(&context.conditions, Keys::Atoms);

    FormView {
        elements: render_elements(&definition.elements, &context, &options, scope),
        valid: form.valid(),
    }
}

/// One form the renderer renders into: the top-level form, or an entry's.
struct Context {
    id: String,
    name: String,
    submitted: bool,
    /// The changeset's params: what a field shows when it has no change,
    /// and what tells a used input from an unused one.
    params: Params,
    changes: Map<String, Value>,
    /// The changes as a condition reads them.
    conditions: Map<String, Value>,
    errors: Vec<(String, String, String)>,
}

#[derive(Clone)]
struct Options<'a> {
    disabled: bool,
    path: Vec<String>,
    sources: Sources<'a>,
}

/// A `Decimal` compares to nothing in a condition: not a number, and
/// equal only to itself.
fn condition_values(changes: &Map<String, Value>, questions: &[&Question]) -> Map<String, Value> {
    let decimals: HashSet<&str> = questions
        .iter()
        .filter(|question| field_type(question) == FieldType::Decimal)
        .map(|question| question.name.as_str())
        .collect();
    changes
        .iter()
        .map(|(field, value)| {
            let value = if decimals.contains(field.as_str()) && value.is_number() {
                json!({"$decimal": value})
            } else {
                value.clone()
            };
            (field.clone(), value)
        })
        .collect()
}

/// `(field, message, validation)`, newest first as Ecto keeps them.
fn errors_by_field(form: &FormChangeset) -> Vec<(String, String, String)> {
    form.changeset
        .errors()
        .iter()
        .rev()
        .map(|error| {
            (
                error.field.clone(),
                error.text(),
                error.validation().to_string(),
            )
        })
        .collect()
}

/// `Visibility.visible_elements/2`, then each element rendered.
fn render_elements(
    elements: &[Element],
    context: &Context,
    options: &Options,
    scope: Scope,
) -> Vec<ElementView> {
    elements
        .iter()
        .filter(|element| {
            let condition = match element {
                Element::Question(question) => question.visible_if.as_deref(),
                Element::Container(container) => container.visible_if.as_deref(),
            };
            condition_met(condition, scope, true)
        })
        .flat_map(|element| match element {
            Element::Question(question) => render_question(question, context, options),
            Element::Container(container) => render_container(container, context, options)
                .into_iter()
                .collect(),
        })
        .collect()
}

impl Context {
    /// A used input, as `Phoenix.Component.used_input?/1` tells it: in the
    /// params, and not marked `_unused_`.
    fn used(&self, field: &str) -> bool {
        used_param(&self.params, field)
    }

    /// The field's errors, once the form has been submitted.
    fn field_errors(&self, field: &str) -> Vec<String> {
        if !self.submitted || !self.used(field) {
            return Vec::new();
        }
        self.errors
            .iter()
            .filter(|(name, _, _)| name == field)
            .map(|(_, message, _)| message.clone())
            .collect()
    }

    /// The field's value, as a Phoenix form gives it: its change, else its
    /// param.
    fn value(&self, question: &Question) -> Value {
        match self.changes.get(&question.name) {
            // Ecto keeps a number as a Decimal, which shows as typed
            Some(_) if field_type(question) == FieldType::Decimal => self
                .params
                .get(&question.name)
                .cloned()
                .unwrap_or(Value::Null),
            Some(change) => change.clone(),
            None => self
                .params
                .get(&question.name)
                .cloned()
                .unwrap_or(Value::Null),
        }
    }

    fn input_name(&self, question: &Question) -> String {
        let suffix = if question.multiple() { "[]" } else { "" };
        format!("{}[{}]{suffix}", self.name, question.name)
    }
}

fn used_param(params: &Params, field: &str) -> bool {
    if field.starts_with("_unused_") {
        return false;
    }
    match params.get(field) {
        None => false,
        Some(_) if params.contains_key(&format!("_unused_{field}")) => false,
        Some(Value::Object(nested)) => nested.keys().any(|key| used_param(nested, key)),
        Some(_) => true,
    }
}

fn render_question(question: &Question, context: &Context, options: &Options) -> Vec<ElementView> {
    if question.no_choices_text.is_some()
        && question.choices_from_question.is_some()
        && resolve_choices(question, &options.sources).is_empty()
    {
        return vec![ElementView::NoChoices {
            label: question.label(),
            required: question.required(),
            required_label: question.required_label(),
            text: elixir::to_string(question.no_choices_text.as_ref().unwrap_or(&Value::Null)),
        }];
    }

    let mut rendered: Vec<ElementView> = field(question, context, options).into_iter().collect();
    if let Some(hidden) = readonly_inputs(question, context, options) {
        rendered.push(hidden);
    }
    rendered
}

/// Disabled by the form or a group, or by the question's `enableIf`,
/// which reads this form's changes.
fn unavailable(question: &Question, context: &Context, options: &Options) -> bool {
    options.disabled
        || !condition_met(
            question.enable_if.as_deref(),
            Scope::new(&context.conditions, Keys::Atoms),
            true,
        )
}

fn field(question: &Question, context: &Context, options: &Options) -> Option<ElementView> {
    if question.kind == "paneldynamic" {
        return Some(ElementView::Nested(nested(question, context, options)));
    }

    let unavailable = unavailable(question, context, options);
    let disabled = question.read_only() || unavailable;
    let value = context.value(question);
    let mut props = Map::new();
    props.insert(
        "id".into(),
        json!(format!("{}_{}", context.id, question.name)),
    );
    props.insert("name".into(), json!(context.input_name(question)));
    props.insert("label".into(), json!(question.label()));
    props.insert("errors".into(), json!(context.field_errors(&question.name)));
    props.insert("required".into(), json!(question.required()));
    props.insert("required_label".into(), json!(question.required_label()));

    let choices = || resolve_choices(question, &options.sources);
    let component = match question.kind.as_str() {
        "text" | "comment" => {
            props.insert("disabled".into(), json!(unavailable));
            props.insert(
                "readonly".into(),
                json!(!unavailable && question.read_only()),
            );
            props.insert("value".into(), text_value(&value));
            props.insert(
                "placeholder".into(),
                question
                    .placeholder
                    .clone()
                    .map(|p| json!(elixir::to_string(&p)))
                    .unwrap_or(Value::Null),
            );
            if question.kind == "comment" {
                props.insert("rows".into(), json!("4"));
                "TextareaInput"
            } else {
                props.insert(
                    "type".into(),
                    json!(question.input_type.as_deref().unwrap_or("text")),
                );
                "TextInput"
            }
        }
        "dropdown" | "tagbox" => {
            let multiple = question.kind == "tagbox";
            props.insert("disabled".into(), json!(disabled));
            props.insert(
                "prompt".into(),
                json!((!multiple).then_some("Select an option...")),
            );
            props.insert("multiple".into(), json!(multiple));
            props.insert(
                "options".into(),
                options_props(&choices(), &value, "selected"),
            );
            "SelectInput"
        }
        "boolean" => {
            let checked = elixir::to_string(&value) == "true";
            props.insert("disabled".into(), json!(disabled));
            props.insert("checked".into(), json!(checked));
            props.insert("label".into(), boolean_label(question));
            "CheckboxInput"
        }
        "radiogroup" | "rating" => {
            let (layout, pairs) = if question.kind == "rating" {
                let pairs = question
                    .rating_values()
                    .into_iter()
                    .map(|number| (json!(number.to_string()), json!(number)))
                    .collect();
                ("horizontal", pairs)
            } else {
                (style(question, true), choices())
            };
            props.insert("disabled".into(), json!(disabled));
            props.insert("layout".into(), json!(layout));
            props.insert("options".into(), options_props(&pairs, &value, "checked"));
            "RadioGroupInput"
        }
        "checkbox" => {
            props.insert("disabled".into(), json!(disabled));
            props.insert("layout".into(), json!(style(question, false)));
            props.insert(
                "options".into(),
                options_props(&choices(), &value, "checked"),
            );
            "CheckboxGroupInput"
        }
        // A type the library doesn't know renders nothing
        _ => return None,
    };

    let description = if question.kind == "boolean" {
        None
    } else {
        question
            .description
            .as_ref()
            .filter(|d| elixir::truthy(d))
            .map(elixir::to_string)
    };

    Some(ElementView::Field {
        name: question.name.clone(),
        component,
        props: Value::Object(props),
        description,
    })
}

/// `metadata.style`: a radio group takes `vertical` or `horizontal`, a
/// checkbox group only `horizontal`, and both are vertical otherwise.
fn style(question: &Question, radio: bool) -> &'static str {
    let style = question
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.get("style"))
        .and_then(Value::as_str);
    match (style, radio) {
        (Some("horizontal"), _) => "horizontal",
        _ => "vertical",
    }
}

/// A checkbox's label shows its description under it, as HTML, the text
/// escaped.
fn boolean_label(question: &Question) -> Value {
    let Some(text) = question.label() else {
        return Value::Null;
    };
    match &question.description {
        Some(description) if !elixir::blank(description) => json!({
            "html": format!(
                r#"{}<br><span class="text-gray-500">{}</span>"#,
                elixir::html_escape(&text),
                elixir::html_escape(&elixir::to_string(description))
            )
        }),
        _ => json!(text),
    }
}

fn text_value(value: &Value) -> Value {
    match value {
        Value::Null => Value::Null,
        other => json!(elixir::to_string(other)),
    }
}

/// Each choice as `{label, value}` strings, marked under `key` when the
/// field's value is, or includes, the choice's.
fn options_props(choices: &[(Value, Value)], value: &Value, key: &str) -> Value {
    let current: Vec<String> = match value {
        Value::Null => Vec::new(),
        Value::Array(values) => values.iter().map(elixir::to_string).collect(),
        other => vec![elixir::to_string(other)],
    };
    choices
        .iter()
        .map(|(label, value)| {
            let value = elixir::to_string(value);
            let marked = current.contains(&value);
            json!({"label": elixir::to_string(label), "value": value, key: marked})
        })
        .collect()
}

/// `readonly_value_inputs/3`: a read-only control other than a text one
/// renders disabled, so hidden inputs submit its value, unless the
/// question is unavailable.
fn readonly_inputs(
    question: &Question,
    context: &Context,
    options: &Options,
) -> Option<ElementView> {
    if question.read_only != Some(Value::Bool(true))
        || ["text", "comment", "paneldynamic", "file"].contains(&question.kind.as_str())
        || unavailable(question, context, options)
    {
        return None;
    }
    let name = context.input_name(question);
    let values: Vec<String> = match context.value(question) {
        Value::Null => Vec::new(),
        Value::Array(values) => values.iter().map(elixir::to_string).collect(),
        value => vec![elixir::to_string(&value)],
    };
    Some(ElementView::Hidden {
        inputs: values
            .into_iter()
            .map(|value| HiddenInput {
                name: name.clone(),
                value,
            })
            .collect(),
    })
}

fn render_container(
    container: &Container,
    context: &Context,
    options: &Options,
) -> Option<ElementView> {
    match container.kind.as_str() {
        "html" => Some(ElementView::Html {
            html: elixir::to_string(container.html.as_ref().unwrap_or(&Value::Null)),
        }),
        "image" => {
            let alt = match &container.title {
                Some(title) if elixir::truthy(title) => elixir::to_string(title),
                _ => container.name.clone(),
            };
            let style = [
                (&container.image_width, "width"),
                (&container.image_height, "height"),
                (&container.image_fit, "object-fit"),
            ]
            .iter()
            .filter_map(|(value, property)| {
                value
                    .as_ref()
                    .filter(|value| elixir::truthy(value))
                    .map(|value| format!("{property}: {};", elixir::to_string(value)))
            })
            .collect::<Vec<_>>()
            .join(" ");
            Some(ElementView::Image {
                src: elixir::to_string(container.image_link.as_ref().unwrap_or(&Value::Null)),
                alt,
                style,
            })
        }
        "panel" => {
            let scope = Scope::new(&context.conditions, Keys::Atoms);
            let mut options = options.clone();
            if !condition_met(container.enable_if.as_deref(), scope, true) {
                options.disabled = true;
            }
            let title = container
                .title
                .as_ref()
                .filter(|title| !elixir::blank(title))
                .map(elixir::to_string);
            Some(ElementView::Group {
                group_type: container.group_type().to_string(),
                name: container.name.clone(),
                title,
                disabled: options.disabled,
                elements: render_elements(
                    container.elements.as_deref().unwrap_or_default(),
                    context,
                    &options,
                    scope,
                ),
            })
        }
        // Element types the library doesn't know render nothing
        _ => None,
    }
}

/// `NestedEntries.nested_form/4`. The entries' changesets are built again
/// here, as the library builds them to render, without the form-level
/// questions it passes when validating.
fn nested(question: &Question, context: &Context, options: &Options) -> NestedView {
    let disabled = question.read_only() || unavailable(question, context, options);
    let children = entry_changesets(question, &context.params, None);
    let count = children.len() as i64;

    let path: Vec<String> = options
        .path
        .iter()
        .cloned()
        .chain([question.name.clone()])
        .collect();
    let mut entry_options = options.clone();
    entry_options.disabled = options.disabled || disabled;

    let show_add = question.allow_add_panel != Some(Value::Bool(false))
        && !disabled
        && question.max_panel_count.is_none_or(|max| count < max);
    let show_remove = question.allow_remove_panel != Some(Value::Bool(false))
        && !disabled
        && count > question.min_panel_count.unwrap_or(0);
    let confirm = (question.confirm_delete == Some(Value::Bool(true))).then(|| {
        question
            .confirm_delete_text
            .as_ref()
            .filter(|text| elixir::truthy(text))
            .map(elixir::to_string)
            .unwrap_or_else(|| "Are you sure you want to delete the record?".to_string())
    });
    let remove_label = text_or(&question.remove_panel_text, "Remove");

    let template = question.template_elements.as_deref().unwrap_or_default();
    let entries = children
        .iter()
        .enumerate()
        .map(|(index, child)| {
            entry(
                question,
                template,
                context,
                &entry_options,
                &path,
                child,
                index,
            )
            .with_remove(show_remove.then(|| RemoveButton {
                label: remove_label.clone(),
                confirm: confirm.clone(),
            }))
        })
        .collect::<Vec<_>>();

    let errors = if context.submitted && context.used(&question.name) {
        context
            .errors
            .iter()
            .filter(|(field, _, validation)| {
                field == &question.name && validation != "paneldynamic"
            })
            .map(|(_, message, _)| message.clone())
            .collect()
    } else {
        Vec::new()
    };

    NestedView {
        name: question.name.clone(),
        path: path.join("."),
        label: question.label(),
        required: question.required(),
        required_label: question.required_label(),
        description: question
            .description
            .as_ref()
            .filter(|d| elixir::truthy(d))
            .map(elixir::to_string),
        add_text: show_add.then(|| text_or(&question.add_panel_text, "Add new")),
        empty_input: format!("{}[{}][__empty__]", context.name, question.name),
        no_entries_text: entries
            .is_empty()
            .then(|| text_or(&question.no_entries_text, "No entries yet.")),
        entries,
        errors,
    }
}

fn text_or(value: &Option<Value>, default: &str) -> String {
    value
        .as_ref()
        .filter(|value| elixir::truthy(value))
        .map(elixir::to_string)
        .unwrap_or_else(|| default.to_string())
}

impl EntryView {
    fn with_remove(mut self, remove: Option<RemoveButton>) -> Self {
        self.remove = remove;
        self
    }
}

/// `NestedEntries.render_entry/5`: the entry as a form of its own, named
/// `post[rooms][0]`, its conditions reading the form's changes with the
/// entry's over them, and its fields' `enableIf` the entry's alone.
fn entry(
    question: &Question,
    template: &[Element],
    parent: &Context,
    options: &Options,
    path: &[String],
    child: &FormChangeset,
    index: usize,
) -> EntryView {
    let changes = child.data();
    let template_questions = list_questions(template);
    let mut all_template = template_questions.clone();
    let id_question = Question {
        name: ID_FIELD.to_string(),
        kind: "text".to_string(),
        ..Question::default()
    };
    all_template.push(&id_question);

    let context = Context {
        id: format!("{}_{}_{index}", parent.id, question.name),
        name: format!("{}[{}][{index}]", parent.name, question.name),
        submitted: parent.submitted,
        params: child.changeset.params().clone(),
        conditions: condition_values(&changes, &all_template),
        changes: changes.clone(),
        errors: errors_by_field(child),
    };

    // The form's changes and the entry's, keyed by strings
    let mut visible: Map<String, Value> = parent.conditions.clone();
    visible.extend(context.conditions.clone());

    let mut options = options.clone();
    options.path = path.iter().cloned().chain([index.to_string()]).collect();
    options.sources.entry_data = changes;
    options.sources.entry_questions = questions_by_name(template);

    let id_input = question
        .generate_ids()
        .then(|| context.value(&id_question))
        .filter(elixir::truthy)
        .map(|value| HiddenInput {
            name: format!("{}[{ID_FIELD}]", context.name),
            value: elixir::to_string(&value),
        });

    let title = question
        .template_title
        .as_ref()
        .filter(|title| !elixir::blank(title))
        .map(|title| elixir::to_string(title).replace("{panelIndex}", &(index + 1).to_string()));

    EntryView {
        index,
        title,
        id_input,
        elements: render_elements(
            template,
            &context,
            &options,
            Scope::new(&visible, Keys::Strings),
        ),
        remove: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::changeset::changeset;

    fn definition() -> Definition {
        Definition::parse(
            r#"{"elements": [
                {"name": "body", "type": "comment", "title": "Write a post", "isRequired": true},
                {"name": "tags", "type": "checkbox", "choices": ["a", {"value": "b", "text": "Bee"}]},
                {"name": "kind", "type": "dropdown", "choices": ["x"], "description": "Pick one"},
                {"name": "score", "type": "rating", "rateMax": 3},
                {"name": "agree", "type": "boolean", "title": false},
                {"name": "email", "type": "text", "inputType": "email", "placeholder": "you@example.com"}
            ]}"#,
        )
        .unwrap()
    }

    fn params(value: Value) -> Params {
        value.as_object().cloned().unwrap()
    }

    fn shown(definition: &Definition, params: Value, submitted: bool) -> FormView {
        let form = changeset(&definition.elements, &self::params(params));
        view(
            definition,
            FormTarget {
                id: "post-form",
                name: "post",
                submitted,
            },
            &form,
        )
    }

    fn props(view: &FormView, index: usize) -> &Value {
        match &view.elements[index] {
            ElementView::Field { props, .. } => props,
            other => panic!("not a field: {other:?}"),
        }
    }

    #[test]
    fn each_question_gets_its_component_and_props() {
        let form = shown(
            &definition(),
            json!({"tags": ["b"], "agree": "true", "body": ""}),
            true,
        );
        let components: Vec<&str> = form
            .elements
            .iter()
            .map(|element| match element {
                ElementView::Field { component, .. } => *component,
                _ => "other",
            })
            .collect();
        assert_eq!(
            components,
            [
                "TextareaInput",
                "CheckboxGroupInput",
                "SelectInput",
                "RadioGroupInput",
                "CheckboxInput",
                "TextInput"
            ]
        );
        assert_eq!(
            props(&form, 0),
            &json!({
                "id": "post-form_body", "name": "post[body]", "label": "Write a post",
                "errors": ["can't be blank"], "required": true, "required_label": "*",
                "disabled": false, "value": "", "placeholder": null, "rows": "4", "readonly": false
            })
        );
        assert_eq!(props(&form, 1)["name"], "post[tags][]");
        assert_eq!(
            props(&form, 1)["options"],
            json!([{"label": "a", "value": "a", "checked": false}, {"label": "Bee", "value": "b", "checked": true}])
        );
        assert_eq!(props(&form, 3)["options"].as_array().unwrap().len(), 3);
        assert_eq!(props(&form, 4)["checked"], true);
        assert!(!form.valid);
    }

    #[test]
    fn errors_show_once_a_submit_failed_on_the_inputs_in_the_params() {
        let typing = shown(&definition(), json!({"body": ""}), false);
        assert_eq!(props(&typing, 0)["errors"], json!([]));

        let submitted = shown(&definition(), json!({"body": ""}), true);
        assert_eq!(props(&submitted, 0)["errors"], json!(["can't be blank"]));

        let required_radio = Definition::parse(r#"{"elements": [{"name": "r", "type": "radiogroup", "isRequired": true, "choices": ["a"]}]}"#).unwrap();
        let unsent = shown(&required_radio, json!({}), true);
        assert!(!unsent.valid);
        assert_eq!(props(&unsent, 0)["errors"], json!([]));
    }

    #[test]
    fn conditions_hide_and_disable_against_the_cast_values() {
        let definition = Definition::parse(
            r#"{"elements": [
                {"name": "pay", "type": "radiogroup", "choices": ["card", "cash"]},
                {"name": "card", "type": "text", "visibleIf": "{pay} = 'card'"},
                {"name": "age", "type": "text", "inputType": "number"},
                {"name": "adult", "type": "text", "visibleIf": "{age} > 17"},
                {"name": "note", "type": "text", "enableIf": "{pay} notempty", "readOnly": true}
            ]}"#,
        )
        .unwrap();
        let form = shown(&definition, json!({"pay": "card", "age": "30"}), false);
        let names: Vec<&str> = form
            .elements
            .iter()
            .filter_map(|element| match element {
                ElementView::Field { name, .. } => Some(name.as_str()),
                _ => None,
            })
            .collect();
        // `{age}` is a Decimal when rendering, so `adult` never shows
        assert_eq!(names, ["pay", "card", "age", "note"]);
        assert_eq!(props(&form, 3)["readonly"], true);
        assert_eq!(props(&form, 3)["disabled"], false);
    }
}
