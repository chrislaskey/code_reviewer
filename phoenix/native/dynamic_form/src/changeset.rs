//! `DynamicForm.Changeset.create_changeset/3`: the params cast to each
//! question's type, then the library's rules, in its order, on the
//! `ecto_changeset` crate so each error is the one Ecto reports. A nested
//! form's entries are changesets of their own, as in `NestedForms`.

use std::collections::{BTreeMap, HashMap};

use ecto_changeset::{Changeset, FieldType, Length, NumberCheck, Params};
use serde_json::{Map, Number, Value};

use crate::carry_forward::prune_values;
use crate::definition::{Element, Question, Validator, list_questions};
use crate::expression::{Keys, Scope, condition_met};
use crate::nested::{list_entries, template_elements};

/// The library's email format, `~r/\A[^\s]+@[^\s]+\.[^\s]+\z/`, as the
/// `regex` crate needs it: PCRE's `\s` without the `u` flag is ASCII
/// whitespace.
const EMAIL_FORMAT: &str = r"\A[^ \t\n\x0B\x0C\r]+@[^ \t\n\x0B\x0C\r]+\.[^ \t\n\x0B\x0C\r]+\z";

/// A form's changeset, and the changesets of each nested form's entries,
/// by the nested form's name.
#[derive(Debug, Clone)]
pub struct FormChangeset {
    pub changeset: Changeset,
    pub entries: BTreeMap<String, Vec<FormChangeset>>,
}

impl FormChangeset {
    pub fn valid(&self) -> bool {
        self.changeset.valid()
    }

    /// `apply_changes/1`: the typed values, nested entries as lists of maps.
    pub fn data(&self) -> Map<String, Value> {
        self.changeset
            .changes()
            .iter()
            .map(|(field, value)| (field.clone(), value.clone()))
            .collect()
    }

    /// Every error, a nested entry's under `name.index.field`.
    pub fn all_errors(&self) -> Vec<ecto_changeset::Error> {
        let mut errors = self.changeset.errors().to_vec();
        for (name, entries) in &self.entries {
            for (index, entry) in entries.iter().enumerate() {
                errors.extend(entry.all_errors().into_iter().map(|mut error| {
                    error.field = format!("{name}.{index}.{}", error.field);
                    error
                }));
            }
        }
        errors
    }
}

/// What the library's `opts` carry into an entry's changeset: the values
/// its conditions read, and the form-level questions a carried-forward
/// source may be among.
#[derive(Debug, Clone, Default)]
pub struct Options<'a> {
    pub visibility_params: Option<&'a Params>,
    pub root_questions: Option<Vec<&'a Question>>,
}

/// Validates `params`, the form's params with string keys.
pub fn changeset(elements: &[Element], params: &Params) -> FormChangeset {
    create_changeset(elements, params, Options::default())
}

/// `create_changeset/3` with its options.
pub fn create_changeset(elements: &[Element], params: &Params, options: Options) -> FormChangeset {
    let questions = list_questions(elements);
    let types: BTreeMap<String, FieldType> = questions
        .iter()
        .map(|question| (question.name.clone(), field_type(question)))
        .collect();
    let visibility = Scope::new(options.visibility_params.unwrap_or(params), Keys::Strings);
    let required: Vec<&str> = questions
        .iter()
        .filter(|question| {
            question.kind != "paneldynamic"
                && (question.required()
                    || condition_met(question.required_if.as_deref(), visibility, false))
                && condition_met(question.visible_if.as_deref(), visibility, true)
        })
        .map(|question| question.name.as_str())
        .collect();

    let decoded = normalize_nested(&questions, &drop_empty_selections(&questions, params));
    let root = options.root_questions.unwrap_or_else(|| questions.clone());
    let scope: Vec<&Question> = questions.iter().chain(root.iter()).copied().collect();
    let names: Vec<&str> = types.keys().map(String::as_str).collect();

    let changeset = Changeset::cast(&Params::new(), &types, &decoded, &names);
    let changeset = prune_values(changeset, &questions, &scope, visibility.values);
    let changeset = changeset.validate_required(&required);
    let changeset = questions.iter().fold(changeset, |changeset, question| {
        validate_question(changeset, question)
    });

    let mut form = FormChangeset {
        changeset,
        entries: BTreeMap::new(),
    };
    for question in questions.iter().filter(|q| q.kind == "paneldynamic") {
        validate_nested(&mut form, question, &decoded, &root);
    }
    form
}

/// The Ecto type the library casts a question as. A type it doesn't know
/// is a string.
pub fn field_type(question: &Question) -> FieldType {
    match question.kind.as_str() {
        "text" if question.input_type.as_deref() == Some("number") => FieldType::Decimal,
        "paneldynamic" => FieldType::Array(Box::new(FieldType::Map)),
        "boolean" => FieldType::Boolean,
        "checkbox" | "tagbox" => FieldType::Array(Box::new(FieldType::String)),
        "rating" => FieldType::Integer,
        _ => FieldType::String,
    }
}

/// A checkbox group submits a hidden `""` so clearing every box still
/// submits the field. The library drops those, and a list left empty
/// becomes null, so a required group with nothing ticked is blank.
fn drop_empty_selections(questions: &[&Question], params: &Params) -> Params {
    let mut params = params.clone();
    for question in questions.iter().filter(|question| question.multiple()) {
        if let Some(Value::Array(values)) = params.get(&question.name) {
            let selected: Vec<Value> = values
                .iter()
                .filter(|value| value.as_str() != Some(""))
                .cloned()
                .collect();
            let value = if selected.is_empty() {
                Value::Null
            } else {
                Value::Array(selected)
            };
            params.insert(question.name.clone(), value);
        }
    }
    params
}

/// `NestedForms.normalize_params/2`: a nested form's value as a list of
/// entries, from the browser's indexed map.
fn normalize_nested(questions: &[&Question], params: &Params) -> Params {
    let mut params = params.clone();
    for question in questions.iter().filter(|q| q.kind == "paneldynamic") {
        match params.get(&question.name) {
            None | Some(Value::Null) => {}
            Some(value) => {
                let entries = list_entries(value);
                params.insert(question.name.clone(), Value::Array(entries));
            }
        }
    }
    params
}

fn validate_question(changeset: Changeset, question: &Question) -> Changeset {
    let field = question.name.as_str();
    let mut changeset = changeset;

    if question.kind == "text" && question.input_type.as_deref() == Some("email") {
        changeset = changeset.validate_format(field, EMAIL_FORMAT, None);
    }

    if question.kind == "rating" {
        let (min, max, _step) = question.rate_range();
        changeset = changeset.validate_number(
            field,
            &[
                NumberCheck::GreaterThanOrEqualTo(Number::from(min)),
                NumberCheck::LessThanOrEqualTo(Number::from(max)),
            ],
            None,
        );
        changeset = validate_rating_step(changeset, question);
    }

    question
        .validators
        .iter()
        .flatten()
        .fold(changeset, |changeset, validator| {
            apply_validator(changeset, field, validator)
        })
}

/// A rating with a `rateStep` of 2 or more offers every step from
/// `rateMin`, so on 1 to 5 with step 2 a 4 is invalid.
fn validate_rating_step(mut changeset: Changeset, question: &Question) -> Changeset {
    let (min, _max, step) = question.rate_range();
    let value = changeset
        .changes()
        .get(&question.name)
        .and_then(Value::as_i64);

    if let Some(value) = value
        && question.rate_step.is_some()
        && step >= 2
        && (value - min) % step != 0
    {
        changeset.add_error(&question.name, "is invalid", "rate_step");
    }
    changeset
}

/// One validator, as the library applies it: each bound is its own
/// validation, so a text validator with both bounds may report twice. A
/// type the library doesn't know does nothing.
fn apply_validator(changeset: Changeset, field: &str, validator: &Validator) -> Changeset {
    let message = validator.text.as_deref();

    match validator.kind.as_str() {
        "text" => {
            let changeset = match validator.min_length {
                Some(min) => changeset.validate_length(field, &Length::default().min(min), message),
                None => changeset,
            };
            match validator.max_length {
                Some(max) => changeset.validate_length(field, &Length::default().max(max), message),
                None => changeset,
            }
        }
        "email" => changeset.validate_format(field, EMAIL_FORMAT, message),
        "numeric" => {
            let changeset = match &validator.min_value {
                Some(min) => changeset.validate_number(
                    field,
                    &[NumberCheck::GreaterThanOrEqualTo(min.clone())],
                    message,
                ),
                None => changeset,
            };
            match &validator.max_value {
                Some(max) => changeset.validate_number(
                    field,
                    &[NumberCheck::LessThanOrEqualTo(max.clone())],
                    message,
                ),
                None => changeset,
            }
        }
        "regex" => match &validator.regex {
            Some(pattern) => changeset.validate_format(field, pattern, message),
            None => changeset,
        },
        _ => changeset,
    }
}

/// `NestedForms.validate_question/3`: each entry's changeset, the entries
/// as the field's change, then `isRequired`, the entry count, and an
/// invalid entry marking the field invalid.
fn validate_nested(
    form: &mut FormChangeset,
    question: &Question,
    params: &Params,
    root: &[&Question],
) {
    let field = question.name.as_str();
    let children = entry_changesets(question, params, Some(root.to_vec()));

    if params.contains_key(field) {
        let applied = children
            .iter()
            .map(|child| Value::Object(child.data()))
            .collect();
        form.changeset.put_change(field, Value::Array(applied));
    }

    let mut changeset = form.changeset.clone();
    if question.is_required == Some(Value::Bool(true)) && children.is_empty() {
        changeset.add_error(field, "can't be blank", "required");
    }
    if let Some(min) = question.min_panel_count.filter(|min| *min != 0) {
        changeset =
            changeset.validate_length(field, &Length::default().min(min.max(0) as usize), None);
    }
    if let Some(max) = question.max_panel_count {
        changeset =
            changeset.validate_length(field, &Length::default().max(max.max(0) as usize), None);
    }
    if !children.iter().all(FormChangeset::valid) {
        changeset.add_error(field, "is invalid", "paneldynamic");
    }

    form.changeset = changeset;
    form.entries.insert(field.to_string(), children);
}

/// `NestedForms.list_entry_changesets/3`: one changeset per entry, against
/// the template. An entry's conditions read its own values over the
/// form's, and as `{panel.field}`. `root` is the form-level questions,
/// which the library passes when it validates and not when it renders.
pub fn entry_changesets(
    question: &Question,
    parent_params: &Params,
    root: Option<Vec<&Question>>,
) -> Vec<FormChangeset> {
    let template = template_elements(question);
    let entries = list_entries(parent_params.get(&question.name).unwrap_or(&Value::Null));

    let children: Vec<FormChangeset> = entries
        .iter()
        .map(|entry| {
            let entry = entry.as_object().cloned().unwrap_or_default();
            let mut context = parent_params.clone();
            context.extend(entry.clone());
            context.extend(
                entry
                    .iter()
                    .map(|(key, value)| (format!("panel.{key}"), value.clone())),
            );

            create_changeset(
                &template,
                &entry,
                Options {
                    visibility_params: Some(&context),
                    root_questions: root.clone(),
                },
            )
        })
        .collect();

    apply_key_duplication(children, question)
}

/// An entry repeating another entry's value for `keyName` gets an error
/// on that field.
fn apply_key_duplication(
    mut children: Vec<FormChangeset>,
    question: &Question,
) -> Vec<FormChangeset> {
    let Some(key) = question.key_name.as_deref().filter(|key| !key.is_empty()) else {
        return children;
    };
    let message = question
        .key_duplication_error
        .as_deref()
        .unwrap_or("value must be unique");

    let values: Vec<Value> = children
        .iter()
        .map(|child| child.changeset.get_field(key))
        .collect();
    let mut counts: HashMap<String, usize> = HashMap::new();
    for value in values
        .iter()
        .filter(|value| !value.is_null() && value.as_str() != Some(""))
    {
        *counts.entry(value.to_string()).or_default() += 1;
    }

    for (child, value) in children.iter_mut().zip(&values) {
        if counts
            .get(&value.to_string())
            .is_some_and(|count| *count > 1)
        {
            child.changeset.add_error(key, message, "key_duplication");
        }
    }
    children
}
