//! A form definition, read as `DynamicForm.Parser.FromData` reads it: the
//! same keys, the same fallbacks, and unknown keys ignored.
//!
//! Where the library would crash on a definition, when it renders or
//! validates it, this refuses it instead, with the reason: a `file`
//! question, which this crate doesn't support yet, an element without a
//! `name`, a condition that isn't a string, and the like.

use serde_json::{Map, Value};

use crate::elixir;

/// The question types the library knows. `file` is one, and refused here.
const QUESTION_TYPES: [&str; 10] = [
    "text",
    "comment",
    "dropdown",
    "radiogroup",
    "boolean",
    "file",
    "checkbox",
    "rating",
    "tagbox",
    "paneldynamic",
];

/// The element types the library knows.
const ELEMENT_TYPES: [&str; 3] = ["html", "panel", "image"];

/// A form: its elements, in the order they render.
#[derive(Debug, Clone, PartialEq)]
pub struct Definition {
    pub id: Option<String>,
    pub title: Option<Value>,
    pub description: Option<Value>,
    pub elements: Vec<Element>,
}

/// A question, which collects a value, or a container element: a panel,
/// html, an image, or a type the library renders nothing for.
#[derive(Debug, Clone, PartialEq)]
pub enum Element {
    Question(Box<Question>),
    Container(Box<Container>),
}

/// One question, a field of the form. `kind` is the type as written: a
/// type the library doesn't know is still a field, cast as a string, and
/// renders nothing.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Question {
    pub name: String,
    pub kind: String,
    pub input_type: Option<String>,
    pub title: Option<Value>,
    pub placeholder: Option<Value>,
    pub description: Option<Value>,
    pub default_value: Option<Value>,
    pub choices: Option<Vec<Choice>>,
    pub validators: Option<Vec<Validator>>,
    pub is_required: Option<Value>,
    /// `requiredLabel`, or `requiredMark` when that is absent; `false` and
    /// `""` are kept as `""`, as the parser keeps them.
    pub required_label: Option<Value>,
    pub required_if: Option<String>,
    pub read_only: Option<Value>,
    pub enable_if: Option<String>,
    pub visible_if: Option<String>,
    pub rate_min: Option<i64>,
    pub rate_max: Option<i64>,
    pub rate_step: Option<i64>,
    pub template_elements: Option<Vec<Element>>,
    pub template_title: Option<Value>,
    pub panel_count: Option<i64>,
    pub min_panel_count: Option<i64>,
    pub max_panel_count: Option<i64>,
    pub allow_add_panel: Option<Value>,
    pub allow_remove_panel: Option<Value>,
    pub add_panel_text: Option<Value>,
    pub remove_panel_text: Option<Value>,
    pub no_entries_text: Option<Value>,
    pub confirm_delete: Option<Value>,
    pub confirm_delete_text: Option<Value>,
    pub key_name: Option<String>,
    pub key_duplication_error: Option<String>,
    pub default_panel_value: Option<Map<String, Value>>,
    pub generate_ids: Option<Value>,
    pub choices_from_question: Option<String>,
    pub choice_values_from_question: Option<String>,
    pub choice_texts_from_question: Option<String>,
    pub choices_from_question_mode: Option<String>,
    pub no_choices_text: Option<Value>,
    pub metadata: Option<Value>,
}

/// A panel, html, an image, or an element type the library renders
/// nothing for.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Container {
    pub name: String,
    pub kind: String,
    pub title: Option<Value>,
    pub group_type: Option<String>,
    pub html: Option<Value>,
    pub elements: Option<Vec<Element>>,
    pub visible_if: Option<String>,
    pub enable_if: Option<String>,
    pub image_link: Option<Value>,
    pub image_width: Option<Value>,
    pub image_height: Option<Value>,
    pub image_fit: Option<Value>,
}

/// A choice, as the parser decodes it: `{"value", "text"}` and the legacy
/// `[label, value]` are labeled; anything else is a plain value.
#[derive(Debug, Clone, PartialEq)]
pub enum Choice {
    Labeled { text: Value, value: Value },
    Plain(Value),
}

impl Choice {
    /// A choice with a label, like a community's name for its id.
    pub fn labeled(text: impl Into<String>, value: impl Into<String>) -> Self {
        Choice::Labeled {
            text: Value::String(text.into()),
            value: Value::String(value.into()),
        }
    }

    /// The `{label, value}` pair `CarryForward.resolve_choices/2` renders.
    pub fn pair(&self) -> (Value, Value) {
        match self {
            Choice::Labeled { text, value } => (text.clone(), value.clone()),
            Choice::Plain(value @ Value::String(_)) => (value.clone(), value.clone()),
            Choice::Plain(value) => (Value::String(elixir::to_string(value)), value.clone()),
        }
    }

    /// The value, as the string the library compares submitted values to.
    pub fn value_text(&self) -> String {
        match self {
            Choice::Labeled { value, .. } | Choice::Plain(value) => elixir::to_string(value),
        }
    }
}

/// One rule of a question's `validators`. A type the library doesn't
/// know is kept, and does nothing, as in the library.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Validator {
    pub kind: String,
    pub min_length: Option<usize>,
    pub max_length: Option<usize>,
    pub min_value: Option<serde_json::Number>,
    pub max_value: Option<serde_json::Number>,
    pub regex: Option<String>,
    /// The message shown instead of the default one.
    pub text: Option<String>,
}

impl Definition {
    /// Reads a definition from its JSON.
    pub fn parse(json: &str) -> Result<Self, String> {
        let value: Value =
            serde_json::from_str(json).map_err(|error| format!("form definition: {error}"))?;
        Self::from_value(&value)
    }

    /// Reads a definition from its JSON, already decoded. A non-empty
    /// `pages` list is flattened into one list of elements, each page's
    /// title becoming an `<h2>` html element, as the library does.
    pub fn from_value(value: &Value) -> Result<Self, String> {
        let data = value
            .as_object()
            .ok_or("form definition: expected an object")?;

        let elements = match data.get("pages") {
            Some(Value::Array(pages)) if !pages.is_empty() => flatten_pages(pages)?,
            _ => match data.get("elements") {
                None | Some(Value::Null) => Vec::new(),
                Some(elements) => elements_list(elements, "elements")?,
            },
        };

        Ok(Definition {
            id: data.get("id").and_then(Value::as_str).map(str::to_string),
            title: present(data.get("title")),
            description: present(data.get("description")),
            elements: elements
                .iter()
                .map(decode_element)
                .collect::<Result<_, _>>()?,
        })
    }

    /// Every question, as `DynamicForm.Instance.Elements.list_questions/1`
    /// lists them: panels are transparent, a nested form's template is not.
    pub fn questions(&self) -> Vec<&Question> {
        list_questions(&self.elements)
    }

    /// The first question named `name`, inside panels too.
    pub fn question(&self, name: &str) -> Option<&Question> {
        self.questions()
            .into_iter()
            .find(|question| question.name == name)
    }

    /// Replaces the choices of the question named `name`, for choices only
    /// known when the form is shown, like the user's communities.
    pub fn put_choices(&mut self, name: &str, choices: Vec<Choice>) -> Result<(), String> {
        let question = find_question_mut(&mut self.elements, name)
            .ok_or_else(|| format!("form definition has no question named {name}"))?;
        question.choices = Some(choices);
        Ok(())
    }
}

/// `Elements.list_questions/1`.
pub fn list_questions(elements: &[Element]) -> Vec<&Question> {
    elements
        .iter()
        .flat_map(|element| match element {
            Element::Question(question) => vec![question.as_ref()],
            Element::Container(container) => match &container.elements {
                Some(nested) => list_questions(nested),
                None => Vec::new(),
            },
        })
        .collect()
}

/// `Elements.questions_by_name/1`: when two questions share a name, the
/// later one wins.
pub fn questions_by_name(elements: &[Element]) -> std::collections::HashMap<&str, &Question> {
    list_questions(elements)
        .into_iter()
        .map(|question| (question.name.as_str(), question))
        .collect()
}

fn find_question_mut<'a>(elements: &'a mut [Element], name: &str) -> Option<&'a mut Question> {
    for element in elements {
        match element {
            Element::Question(question) if question.name == name => return Some(question),
            Element::Container(container) => {
                if let Some(question) = container
                    .elements
                    .as_mut()
                    .and_then(|nested| find_question_mut(nested, name))
                {
                    return Some(question);
                }
            }
            _ => {}
        }
    }
    None
}

impl Question {
    /// The label, as `DynamicForm.Instance.label_text/1` gives it: no
    /// title is the capitalized name, a blank one is none.
    pub fn label(&self) -> Option<String> {
        match &self.title {
            None => Some(elixir::capitalize(&self.name)),
            Some(title) if elixir::blank(title) => None,
            Some(title) => Some(elixir::to_string(title)),
        }
    }

    /// The mark beside a required question's label, as
    /// `DynamicForm.Instance.required_label_text/1` gives it.
    pub fn required_label(&self) -> Option<String> {
        match &self.required_label {
            None => Some("*".to_string()),
            Some(label) if elixir::blank(label) => None,
            Some(label) => Some(elixir::to_string(label)),
        }
    }

    /// `!!question.isRequired`.
    pub fn required(&self) -> bool {
        self.is_required.as_ref().is_some_and(elixir::truthy)
    }

    /// `question.readOnly` as a condition.
    pub fn read_only(&self) -> bool {
        self.read_only.as_ref().is_some_and(elixir::truthy)
    }

    /// A question whose value is a list, submitted as `name[]`.
    pub fn multiple(&self) -> bool {
        matches!(self.kind.as_str(), "checkbox" | "tagbox")
    }

    /// `NestedForms.generate_ids?/1`: on unless `generateIds` is `false`.
    pub fn generate_ids(&self) -> bool {
        self.generate_ids != Some(Value::Bool(false))
    }

    /// `rateMin`, `rateMax` and `rateStep`, 1 to 5 by 1 unless set.
    pub fn rate_range(&self) -> (i64, i64, i64) {
        (
            self.rate_min.unwrap_or(1),
            self.rate_max.unwrap_or(5),
            self.rate_step.unwrap_or(1),
        )
    }

    /// A rating's choices, from `rateMin` up to `rateMax` by `rateStep`.
    pub fn rating_values(&self) -> Vec<i64> {
        let (min, max, step) = self.rate_range();
        std::iter::successors(Some(min), |number| Some(number + step))
            .take_while(|number| *number <= max)
            .collect()
    }

    /// The question's own choices, as `{label, value}` pairs.
    pub fn own_choices(&self) -> Vec<(Value, Value)> {
        self.choices.iter().flatten().map(Choice::pair).collect()
    }
}

impl Container {
    /// A panel's group type: `horizontal` unless set.
    pub fn group_type(&self) -> &str {
        self.group_type.as_deref().unwrap_or("horizontal")
    }
}

fn flatten_pages(pages: &[Value]) -> Result<Vec<Value>, String> {
    let mut elements = Vec::new();
    for page in pages {
        let page = page
            .as_object()
            .ok_or("form definition: a page is an object")?;
        if let Some(Value::String(title)) = page.get("title")
            && !title.is_empty()
        {
            let name = match page.get("name") {
                None => "page".to_string(),
                Some(name) => elixir::to_string(name),
            };
            elements.push(serde_json::json!({
                "type": "html",
                "name": format!("{name}-title"),
                "html": format!("<h2>{}</h2>", elixir::html_escape(title)),
            }));
        }
        match page.get("elements") {
            None | Some(Value::Null) => {}
            Some(page_elements) => elements.extend(elements_list(page_elements, "elements")?),
        }
    }
    Ok(elements)
}

fn elements_list(value: &Value, key: &str) -> Result<Vec<Value>, String> {
    value
        .as_array()
        .cloned()
        .ok_or_else(|| format!("form definition: {key} is a list"))
}

/// `FromData.decode_element/1`: a known question type is a question, a
/// known element type a container; otherwise `choices` or `isRequired`
/// make it a question, and anything else is a container.
fn decode_element(value: &Value) -> Result<Element, String> {
    let data = value
        .as_object()
        .ok_or("form definition: an element is an object")?;
    let kind = data.get("type").and_then(Value::as_str);

    let question = match kind {
        Some(kind) if QUESTION_TYPES.contains(&kind) => true,
        Some(kind) if ELEMENT_TYPES.contains(&kind) => false,
        _ => data.contains_key("choices") || data.contains_key("isRequired"),
    };

    if question {
        decode_question(data).map(|question| Element::Question(Box::new(question)))
    } else {
        decode_container(data).map(|container| Element::Container(Box::new(container)))
    }
}

fn decode_question(data: &Map<String, Value>) -> Result<Question, String> {
    let name = name(data)?;
    let kind = match data.get("type") {
        Some(Value::String(kind)) => kind.clone(),
        Some(_) | None => {
            return Err(format!("form definition: question {name} has no type"));
        }
    };
    if kind == "file" {
        return Err(format!(
            "form definition: question {name} is a file upload, which the desktop app doesn't support yet"
        ));
    }

    let question = Question {
        input_type: string(data, "inputType", &name)?,
        title: present(data.get("title")),
        placeholder: present(data.get("placeholder")),
        description: present(data.get("description")),
        default_value: present(data.get("defaultValue")),
        choices: choices(data.get("choices"), &name)?,
        validators: validators(data.get("validators"), &name)?,
        is_required: present(data.get("isRequired")),
        required_label: required_label(data),
        required_if: condition(data, "requiredIf", &name)?,
        read_only: present(data.get("readOnly")),
        enable_if: condition(data, "enableIf", &name)?,
        visible_if: condition(data, "visibleIf", &name)?,
        rate_min: integer(data, "rateMin", &name)?,
        rate_max: integer(data, "rateMax", &name)?,
        rate_step: integer(data, "rateStep", &name)?,
        template_elements: template_elements(data)?,
        template_title: present(data.get("templateTitle")),
        panel_count: integer(data, "panelCount", &name)?,
        min_panel_count: integer(data, "minPanelCount", &name)?,
        max_panel_count: integer(data, "maxPanelCount", &name)?,
        allow_add_panel: present(data.get("allowAddPanel")),
        allow_remove_panel: present(data.get("allowRemovePanel")),
        add_panel_text: either(data, "addPanelText", "panelAddText"),
        remove_panel_text: either(data, "removePanelText", "panelRemoveText"),
        no_entries_text: present(data.get("noEntriesText")),
        confirm_delete: present(data.get("confirmDelete")),
        confirm_delete_text: present(data.get("confirmDeleteText")),
        key_name: string(data, "keyName", &name)?,
        key_duplication_error: string(data, "keyDuplicationError", &name)?,
        default_panel_value: match present(data.get("defaultPanelValue")) {
            None => None,
            Some(Value::Object(map)) => Some(map),
            Some(_) => {
                return Err(format!(
                    "form definition: question {name}'s defaultPanelValue is a map"
                ));
            }
        },
        generate_ids: present(data.get("generateIds")),
        choices_from_question: string(data, "choicesFromQuestion", &name)?,
        choice_values_from_question: string(data, "choiceValuesFromQuestion", &name)?,
        choice_texts_from_question: string(data, "choiceTextsFromQuestion", &name)?,
        choices_from_question_mode: string(data, "choicesFromQuestionMode", &name)?,
        no_choices_text: present(data.get("noChoicesText")),
        metadata: present(data.get("metadata")),
        name,
        kind,
    };

    check_question(&question)?;
    Ok(question)
}

/// What the library would crash on when it renders or validates.
fn check_question(question: &Question) -> Result<(), String> {
    let name = &question.name;
    if question.kind == "rating" && question.rate_range().2 < 1 {
        // `Stream.iterate/2` never reaches `rateMax`, so the render hangs
        return Err(format!(
            "form definition: rating {name} has a rateStep below 1"
        ));
    }
    if let Some(mode) = &question.choices_from_question_mode
        && question.choices_from_question.is_some()
        && !["all", "selected", "unselected"].contains(&mode.as_str())
    {
        return Err(format!(
            "form definition: question {name} has choicesFromQuestionMode {mode:?}, expected \"all\", \"selected\" or \"unselected\""
        ));
    }
    for validator in question.validators.iter().flatten() {
        if validator.kind == "regex"
            && let Some(pattern) = &validator.regex
            && regex::Regex::new(pattern).is_err()
        {
            return Err(format!(
                "form definition: question {name}'s regex {pattern:?} doesn't compile in the regex crate"
            ));
        }
    }
    Ok(())
}

fn decode_container(data: &Map<String, Value>) -> Result<Container, String> {
    let name = name(data)?;
    let kind = match data.get("type") {
        None => "html".to_string(),
        Some(kind) => elixir::to_string(kind),
    };
    let group_type = string(data, "groupType", &name)?;
    if kind == "panel"
        && let Some(group_type) = &group_type
        && group_type != "horizontal"
        && group_type != "vertical"
    {
        return Err(format!(
            "form definition: panel {name} has groupType {group_type:?}, expected \"horizontal\" or \"vertical\""
        ));
    }

    Ok(Container {
        title: present(data.get("title")),
        html: present(data.get("html")),
        elements: match data.get("elements") {
            None | Some(Value::Null) => None,
            Some(elements) => Some(
                elements_list(elements, "elements")?
                    .iter()
                    .map(decode_element)
                    .collect::<Result<_, _>>()?,
            ),
        },
        visible_if: condition(data, "visibleIf", &name)?,
        enable_if: condition(data, "enableIf", &name)?,
        image_link: present(data.get("imageLink")),
        image_width: present(data.get("imageWidth")),
        image_height: present(data.get("imageHeight")),
        image_fit: present(data.get("imageFit")),
        group_type,
        name,
        kind,
    })
}

fn template_elements(data: &Map<String, Value>) -> Result<Option<Vec<Element>>, String> {
    let value = match either(data, "templateElements", "questions") {
        None => return Ok(None),
        Some(value) => value,
    };
    elements_list(&value, "templateElements")?
        .iter()
        .map(decode_element)
        .collect::<Result<_, _>>()
        .map(Some)
}

fn name(data: &Map<String, Value>) -> Result<String, String> {
    match data.get("name") {
        Some(Value::String(name)) => Ok(name.clone()),
        Some(Value::Null) | None => Err("form definition: an element has no name".to_string()),
        Some(name) => Err(format!("form definition: the name {name} is not a string")),
    }
}

/// A key that is present and not null, as `Map.get/2` gives it.
fn present(value: Option<&Value>) -> Option<Value> {
    value.filter(|value| !value.is_null()).cloned()
}

/// `Map.get(data, a) || Map.get(data, b)`.
fn either(data: &Map<String, Value>, first: &str, second: &str) -> Option<Value> {
    present(data.get(first))
        .filter(elixir::truthy)
        .or_else(|| present(data.get(second)))
}

fn string(data: &Map<String, Value>, key: &str, name: &str) -> Result<Option<String>, String> {
    match present(data.get(key)) {
        None => Ok(None),
        Some(Value::String(text)) => Ok(Some(text)),
        Some(other) => Err(format!(
            "form definition: question {name}'s {key} is {other}, not a string"
        )),
    }
}

/// A `visibleIf`, `enableIf` or `requiredIf`, which the library only
/// evaluates as a string.
fn condition(data: &Map<String, Value>, key: &str, name: &str) -> Result<Option<String>, String> {
    string(data, key, name)
}

fn integer(data: &Map<String, Value>, key: &str, name: &str) -> Result<Option<i64>, String> {
    match present(data.get(key)) {
        None => Ok(None),
        Some(value) => value.as_i64().map(Some).ok_or_else(|| {
            format!("form definition: question {name}'s {key} is {value}, not an integer")
        }),
    }
}

/// `FromData.decode_required_label/1`.
fn required_label(data: &Map<String, Value>) -> Option<Value> {
    let label = present(data.get("requiredLabel")).or_else(|| present(data.get("requiredMark")));
    match label {
        Some(Value::Bool(false)) => Some(Value::String(String::new())),
        label => label,
    }
}

/// `FromData.decode_choices/1`. A map without both `value` and `text`, or
/// a list that isn't a legacy pair, is a value the library can't show.
fn choices(value: Option<&Value>, name: &str) -> Result<Option<Vec<Choice>>, String> {
    let choices = match present(value) {
        None => return Ok(None),
        Some(Value::Array(choices)) => choices,
        Some(_) => {
            return Err(format!(
                "form definition: question {name}'s choices is a list"
            ));
        }
    };
    choices
        .into_iter()
        .map(|choice| match choice {
            Value::Object(ref map) if map.contains_key("value") && map.contains_key("text") => {
                Ok(Choice::Labeled {
                    text: map["text"].clone(),
                    value: map["value"].clone(),
                })
            }
            Value::Array(ref pair) if pair.len() == 2 && pair[0].is_string() => {
                Ok(Choice::Labeled {
                    text: pair[0].clone(),
                    value: pair[1].clone(),
                })
            }
            Value::Object(_) | Value::Array(_) => Err(format!(
                "form definition: question {name} has a choice {choice}, which the library can't show"
            )),
            value => Ok(Choice::Plain(value)),
        })
        .collect::<Result<_, _>>()
        .map(Some)
}

fn validators(value: Option<&Value>, name: &str) -> Result<Option<Vec<Validator>>, String> {
    let validators = match present(value) {
        None => return Ok(None),
        Some(Value::Array(validators)) => validators,
        Some(_) => {
            return Err(format!(
                "form definition: question {name}'s validators is a list"
            ));
        }
    };
    validators
        .iter()
        .map(|validator| {
            let data = validator.as_object().ok_or_else(|| {
                format!("form definition: question {name} has a validator that isn't an object")
            })?;
            let kind = match data.get("type") {
                Some(Value::String(kind)) => kind.clone(),
                _ => {
                    return Err(format!(
                        "form definition: question {name} has a validator with no type"
                    ));
                }
            };
            let length = |key: &str| match present(data.get(key)) {
                None => Ok(None),
                Some(value) => value.as_u64().map(|n| Some(n as usize)).ok_or_else(|| {
                    format!("form definition: question {name}'s {key} is {value}, not a length")
                }),
            };
            let number = |key: &str| match present(data.get(key)) {
                None => Ok(None),
                Some(Value::Number(number)) => Ok(Some(number)),
                Some(value) => Err(format!(
                    "form definition: question {name}'s {key} is {value}, not a number"
                )),
            };
            Ok(Validator {
                min_length: length("minLength")?,
                max_length: length("maxLength")?,
                min_value: number("minValue")?,
                max_value: number("maxValue")?,
                regex: string(data, "regex", name)?,
                text: string(data, "text", name)?,
                kind,
            })
        })
        .collect::<Result<_, _>>()
        .map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn question(json: &str) -> Question {
        match Definition::parse(&format!(r#"{{"elements": [{json}]}}"#))
            .expect("a definition")
            .elements
            .remove(0)
        {
            Element::Question(question) => *question,
            other => panic!("not a question: {other:?}"),
        }
    }

    #[test]
    fn a_question_without_a_title_is_labeled_with_its_name() {
        let label = |json: &str| question(json).label();
        assert_eq!(
            label(r#"{"name": "first_NAME", "type": "text"}"#).as_deref(),
            Some("First_name")
        );
        assert_eq!(
            label(r#"{"name": "a", "type": "text", "title": null}"#).as_deref(),
            Some("A")
        );
        assert_eq!(
            label(r#"{"name": "a", "type": "text", "title": "Body"}"#).as_deref(),
            Some("Body")
        );
        assert_eq!(label(r#"{"name": "a", "type": "text", "title": ""}"#), None);
        assert_eq!(
            label(r#"{"name": "a", "type": "text", "title": false}"#),
            None
        );
    }

    #[test]
    fn the_required_mark_is_a_star_unless_the_definition_sets_one() {
        let mark = |json: &str| question(json).required_label();
        assert_eq!(
            mark(r#"{"name": "a", "type": "text"}"#).as_deref(),
            Some("*")
        );
        assert_eq!(
            mark(r#"{"name": "a", "type": "text", "requiredLabel": "(required)"}"#).as_deref(),
            Some("(required)")
        );
        assert_eq!(
            mark(r#"{"name": "a", "type": "text", "requiredMark": "!"}"#).as_deref(),
            Some("!")
        );
        assert_eq!(
            mark(r#"{"name": "a", "type": "text", "requiredLabel": ""}"#),
            None
        );
        assert_eq!(
            mark(r#"{"name": "a", "type": "text", "requiredLabel": false}"#),
            None
        );
        assert_eq!(
            mark(r#"{"name": "a", "type": "text", "requiredMark": false}"#),
            None
        );
    }

    #[test]
    fn choices_are_labeled_values_or_plain_ones() {
        let pairs: Vec<(String, String)> = question(
            r#"{"name": "a", "type": "dropdown", "choices": ["red", 2, 1.5, true, {"value": "b", "text": "Blue"}, ["Green", "g"]]}"#,
        )
        .own_choices()
        .iter()
        .map(|(text, value)| (elixir::to_string(text), elixir::to_string(value)))
        .collect();
        let expected = [
            ("red", "red"),
            ("2", "2"),
            ("1.5", "1.5"),
            ("true", "true"),
            ("Blue", "b"),
            ("Green", "g"),
        ];
        assert_eq!(
            pairs,
            expected.map(|(text, value)| (text.to_string(), value.to_string()))
        );
    }

    #[test]
    fn a_rating_offers_its_numbers() {
        let rating = question(
            r#"{"name": "a", "type": "rating", "rateMin": 0, "rateMax": 10, "rateStep": 4}"#,
        );
        assert_eq!(rating.rating_values(), [0, 4, 8]);
    }

    #[test]
    fn elements_are_typed_as_the_library_types_them() {
        let definition = Definition::parse(
            r#"{"elements": [
                {"name": "intro", "html": "<p>Hi</p>"},
                {"name": "custom", "type": "slider"},
                {"name": "with_choices", "type": "slider", "choices": []},
                {"name": "box", "type": "panel", "elements": [{"name": "inner", "type": "text"}]}
            ]}"#,
        )
        .unwrap();

        let kinds: Vec<String> = definition
            .elements
            .iter()
            .map(|element| match element {
                Element::Question(question) => format!("question {}", question.kind),
                Element::Container(container) => format!("container {}", container.kind),
            })
            .collect();
        assert_eq!(
            kinds,
            [
                "container html",
                "container slider",
                "question slider",
                "container panel"
            ]
        );
        let names: Vec<&str> = definition
            .questions()
            .iter()
            .map(|q| q.name.as_str())
            .collect();
        assert_eq!(names, ["with_choices", "inner"]);
    }

    #[test]
    fn pages_flatten_with_their_titles_as_headings() {
        let definition = Definition::parse(
            r#"{"pages": [
                {"name": "one", "title": "Tom & Jerry", "elements": [{"name": "a", "type": "text"}]},
                {"elements": [{"name": "b", "type": "text"}]}
            ]}"#,
        )
        .unwrap();
        assert_eq!(definition.elements.len(), 3);
        match &definition.elements[0] {
            Element::Container(container) => {
                assert_eq!(container.name, "one-title");
                assert_eq!(
                    container.html,
                    Some(Value::from("<h2>Tom &amp; Jerry</h2>"))
                );
            }
            other => panic!("not a heading: {other:?}"),
        }
    }

    #[test]
    fn unknown_keys_and_validators_are_ignored_as_in_the_library() {
        let question = question(
            r#"{"name": "a", "type": "text", "visible": false, "validators": [{"type": "expression", "expression": "1"}]}"#,
        );
        assert_eq!(question.validators.unwrap()[0].kind, "expression");
    }

    #[test]
    fn what_the_library_crashes_on_or_the_crate_skips_is_refused() {
        let refused = [
            r#"{"elements": [{"name": "a", "type": "file"}]}"#,
            r#"{"elements": [{"type": "text"}]}"#,
            r#"{"elements": [{"name": "a", "choices": []}]}"#,
            r#"{"elements": [{"name": "a", "type": "text", "visibleIf": 1}]}"#,
            r#"{"elements": [{"name": "a", "type": "rating", "rateStep": 0}]}"#,
            r#"{"elements": [{"name": "a", "type": "panel", "groupType": "grid"}]}"#,
            r#"{"elements": [{"name": "a", "type": "dropdown", "choices": [{"value": 1}]}]}"#,
            r#"{"elements": [{"name": "a", "type": "dropdown", "choicesFromQuestion": "b", "choicesFromQuestionMode": "some"}]}"#,
        ];
        for json in refused {
            assert!(Definition::parse(json).is_err(), "parsed {json}");
        }
    }

    #[test]
    fn choices_can_be_put_once_known_inside_panels_too() {
        let mut definition = Definition::parse(
            r#"{"elements": [{"name": "p", "type": "panel", "elements": [{"name": "ids", "type": "checkbox", "choices": []}]}]}"#,
        )
        .unwrap();
        definition
            .put_choices("ids", vec![Choice::labeled("X", "x")])
            .unwrap();
        assert_eq!(definition.question("ids").unwrap().own_choices().len(), 1);
        assert!(definition.put_choices("missing", vec![]).is_err());
    }
}
