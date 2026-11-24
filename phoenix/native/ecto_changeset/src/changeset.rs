//! `Ecto.Changeset`'s cast and validations, with its semantics, messages
//! and error options, so the errors are the ones the Phoenix app reports.
//!
//! `cast` turns params into typed changes, and each `validate_*` adds the
//! errors Ecto would. As in Ecto, a validation runs on a change only when
//! the field changed to a non-null value, and `validate_confirmation` and
//! `validate_acceptance` read the raw params.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;
use serde_json::Value;
use unicode_segmentation::UnicodeSegmentation;

/// Params or data: a JSON object keyed by field name.
pub type Params = serde_json::Map<String, Value>;

/// The Ecto type of a field, as far as the cast needs it. The unit
/// variants are Ecto's built-in types.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    Id,
    String,
    Integer,
    Float,
    Decimal,
    Boolean,
    Binary,
    Date,
    Time,
    UtcDatetime,
    UtcDatetimeUsec,
    NaiveDatetime,
    NaiveDatetimeUsec,
    Map,
    /// `Ecto.UUID`.
    Uuid,
    /// `Ecto.Enum`, with the strings it casts from.
    Enum(Vec<String>),
    Array(Box<FieldType>),
}

/// One error as Ecto reports it: the message with `%{count}` and the like
/// left as is, and the options Ecto keeps with it, in Ecto's order.
/// `validation` is always among them; a length adds `count`, `kind` and
/// `type`, a number `kind` and `number`, an inclusion `enum`, a cast
/// `type`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Error {
    pub field: String,
    pub message: String,
    pub options: Vec<(String, Value)>,
}

impl Error {
    /// The `validation` option: `required`, `length`, `cast` and so on.
    pub fn validation(&self) -> &str {
        self.option("validation")
            .and_then(Value::as_str)
            .unwrap_or("")
    }

    pub fn option(&self, key: &str) -> Option<&Value> {
        self.options
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value)
    }

    /// The message with its options filled in, as a form shows it:
    /// `should be at most 80 character(s)`.
    pub fn text(&self) -> String {
        static PLACEHOLDER: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"%\{(\w+)\}").expect("placeholder regex"));
        PLACEHOLDER
            .replace_all(&self.message, |captures: &regex::Captures| {
                match self.option(&captures[1]) {
                    Some(Value::String(text)) => text.clone(),
                    Some(value) => value.to_string(),
                    None => captures[0].to_string(),
                }
            })
            .into_owned()
    }
}

/// The `:is`, `:min`, `:max` and `:count` options of `validate_length/3`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Length {
    pub is: Option<usize>,
    pub min: Option<usize>,
    pub max: Option<usize>,
    pub count: Count,
}

impl Length {
    pub fn is(mut self, is: usize) -> Self {
        self.is = Some(is);
        self
    }

    pub fn min(mut self, min: usize) -> Self {
        self.min = Some(min);
        self
    }

    pub fn max(mut self, max: usize) -> Self {
        self.max = Some(max);
        self
    }

    pub fn count(mut self, count: Count) -> Self {
        self.count = count;
        self
    }
}

/// What `validate_length/3` counts in a string.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Count {
    #[default]
    Graphemes,
    Codepoints,
    Bytes,
}

/// One option of `validate_number/3`, such as `greater_than: 0`.
#[derive(Debug, Clone, PartialEq)]
pub enum NumberCheck {
    LessThan(serde_json::Number),
    GreaterThan(serde_json::Number),
    LessThanOrEqualTo(serde_json::Number),
    GreaterThanOrEqualTo(serde_json::Number),
    EqualTo(serde_json::Number),
    NotEqualTo(serde_json::Number),
}

/// A database constraint the changeset declares, such as
/// `unique_constraint/3`. The database checks it, not this crate, so a
/// local store has to check it itself before inserting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Constraint {
    /// `unique`, `foreign_key`, `check`, `exclusion` or `no_assoc`.
    pub kind: String,
    pub fields: Vec<String>,
    pub name: Option<String>,
    pub message: Option<String>,
}

/// A changeset being validated. Build one with `cast`, chain the
/// validations, then read `errors`, `valid` or `changes`.
#[derive(Debug, Clone)]
pub struct Changeset {
    data: Params,
    params: Params,
    changes: BTreeMap<String, Value>,
    errors: Vec<Error>,
    required: Vec<String>,
}

impl Changeset {
    /// `Ecto.Changeset.cast/4`: every permitted param is cast to its type
    /// and kept as a change when it differs from `data`. A string that is
    /// blank once trimmed is empty, Ecto's default, and empty strings are
    /// dropped from a list.
    pub fn cast(
        data: &Params,
        types: &BTreeMap<String, FieldType>,
        params: &Params,
        permitted: &[&str],
    ) -> Self {
        let mut changeset = Changeset {
            data: data.clone(),
            params: params.clone(),
            changes: BTreeMap::new(),
            errors: Vec::new(),
            required: Vec::new(),
        };

        for field in permitted {
            let Some(param) = params.get(*field) else {
                continue;
            };
            let Some(field_type) = types.get(*field) else {
                changeset.add_error(field, "has no type in the generated Rust", "rust");
                continue;
            };
            let current = data.get(*field).cloned().unwrap_or(Value::Null);
            let param = filter_empty(field_type, param);

            match cast_value(field_type, &param) {
                Some(value) if values_equal(&current, &value) => {}
                Some(value) => {
                    changeset.changes.insert(field.to_string(), value);
                }
                None => changeset.errors.push(Error {
                    field: field.to_string(),
                    message: "is invalid".to_string(),
                    options: vec![
                        (
                            "type".to_string(),
                            serde_json::to_value(field_type).unwrap_or(Value::Null),
                        ),
                        ("validation".to_string(), Value::from("cast")),
                    ],
                }),
            }
        }

        changeset
    }

    pub fn valid(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn errors(&self) -> &[Error] {
        &self.errors
    }

    /// The typed changes, keyed by field.
    pub fn changes(&self) -> &BTreeMap<String, Value> {
        &self.changes
    }

    pub fn required(&self) -> &[String] {
        &self.required
    }

    /// The params the changeset was cast from.
    pub fn params(&self) -> &Params {
        &self.params
    }

    /// `put_change/3`: the value becomes the field's change, unless it
    /// equals the field's value in `data`, which removes the change.
    pub fn put_change(&mut self, field: &str, value: Value) {
        let current = self.data.get(field).cloned().unwrap_or(Value::Null);
        if values_equal(&current, &value) {
            self.changes.remove(field);
        } else {
            self.changes.insert(field.to_string(), value);
        }
    }

    /// `delete_change/2`.
    pub fn delete_change(&mut self, field: &str) {
        self.changes.remove(field);
    }

    /// The field's change, or its value in `data`.
    pub fn get_field(&self, field: &str) -> Value {
        self.changes
            .get(field)
            .or_else(|| self.data.get(field))
            .cloned()
            .unwrap_or(Value::Null)
    }

    /// `validate_required/3`: a field is missing when it is null or an empty
    /// string in both the changes and the data. A field with an error
    /// already is skipped, and a missing field's change is dropped.
    pub fn validate_required(mut self, fields: &[&str]) -> Self {
        for field in fields {
            self.required.push(field.to_string());
            let has_error = self.errors.iter().any(|error| error.field == *field);
            if !has_error && is_missing(&self.get_field(field)) {
                self.changes.remove(*field);
                self.add_error(field, "can't be blank", "required");
            }
        }
        self
    }

    /// `validate_format/4`, with the `regex` crate: a pattern with
    /// lookaround or backreferences does not compile and reports as an
    /// error on the field instead of being skipped silently.
    pub fn validate_format(mut self, field: &str, pattern: &str, message: Option<&str>) -> Self {
        let Some(value) = self.change(field) else {
            return self;
        };
        let regex = match Regex::new(pattern) {
            Ok(regex) => regex,
            Err(_) => {
                let text = format!("has a pattern the regex crate cannot compile: {pattern}");
                self.add_error(field, &text, "rust");
                return self;
            }
        };
        match value.as_str() {
            Some(text) if regex.is_match(text) => {}
            Some(_) => self.add_error(field, message.unwrap_or("has invalid format"), "format"),
            None => self.add_error(field, "validate_format expects a string", "rust"),
        }
        self
    }

    /// `validate_length/3` over a string, list or map change.
    pub fn validate_length(mut self, field: &str, length: &Length, message: Option<&str>) -> Self {
        let Some(value) = self.change(field) else {
            return self;
        };
        let (kind, actual) = match (&value, length.count) {
            (Value::String(text), Count::Graphemes) => ("string", text.graphemes(true).count()),
            (Value::String(text), Count::Codepoints) => ("string", text.chars().count()),
            (Value::String(text), Count::Bytes) => ("binary", text.len()),
            (Value::Array(items), _) => ("list", items.len()),
            (Value::Object(entries), _) => ("map", entries.len()),
            _ => {
                self.add_error(
                    field,
                    "validate_length expects a string, list or map",
                    "rust",
                );
                return self;
            }
        };
        let (verb, unit) = match kind {
            "string" => ("should be", "character(s)"),
            "binary" => ("should be", "byte(s)"),
            _ => ("should have", "item(s)"),
        };

        let failed = length
            .is
            .filter(|is| actual != *is)
            .map(|is| (format!("{verb} %{{count}} {unit}"), "is", is))
            .or_else(|| {
                length
                    .min
                    .filter(|min| actual < *min)
                    .map(|min| (format!("{verb} at least %{{count}} {unit}"), "min", min))
            })
            .or_else(|| {
                length
                    .max
                    .filter(|max| actual > *max)
                    .map(|max| (format!("{verb} at most %{{count}} {unit}"), "max", max))
            });

        if let Some((default, check, count)) = failed {
            self.errors.push(Error {
                field: field.to_string(),
                message: message.unwrap_or(&default).to_string(),
                options: vec![
                    ("count".to_string(), Value::from(count)),
                    ("validation".to_string(), Value::from("length")),
                    ("kind".to_string(), Value::from(check)),
                    ("type".to_string(), Value::from(kind)),
                ],
            });
        }
        self
    }

    /// `validate_number/3`: the checks run in order and the first failing
    /// one reports.
    pub fn validate_number(
        mut self,
        field: &str,
        checks: &[NumberCheck],
        message: Option<&str>,
    ) -> Self {
        let Some(value) = self.change(field) else {
            return self;
        };
        let Some(actual) = value.as_f64() else {
            self.add_error(field, "validate_number expects a number", "rust");
            return self;
        };

        for check in checks {
            let (holds, default, kind, target) = match check {
                NumberCheck::LessThan(n) => (
                    actual < as_f64(n),
                    "must be less than %{number}",
                    "less_than",
                    n,
                ),
                NumberCheck::GreaterThan(n) => (
                    actual > as_f64(n),
                    "must be greater than %{number}",
                    "greater_than",
                    n,
                ),
                NumberCheck::LessThanOrEqualTo(n) => (
                    actual <= as_f64(n),
                    "must be less than or equal to %{number}",
                    "less_than_or_equal_to",
                    n,
                ),
                NumberCheck::GreaterThanOrEqualTo(n) => (
                    actual >= as_f64(n),
                    "must be greater than or equal to %{number}",
                    "greater_than_or_equal_to",
                    n,
                ),
                NumberCheck::EqualTo(n) => (
                    actual == as_f64(n),
                    "must be equal to %{number}",
                    "equal_to",
                    n,
                ),
                NumberCheck::NotEqualTo(n) => (
                    actual != as_f64(n),
                    "must be not equal to %{number}",
                    "not_equal_to",
                    n,
                ),
            };
            if !holds {
                self.errors.push(Error {
                    field: field.to_string(),
                    message: message.unwrap_or(default).to_string(),
                    options: vec![
                        ("validation".to_string(), Value::from("number")),
                        ("kind".to_string(), Value::from(kind)),
                        ("number".to_string(), Value::Number(target.clone())),
                    ],
                });
                break;
            }
        }
        self
    }

    /// `validate_inclusion/4`.
    pub fn validate_inclusion(
        mut self,
        field: &str,
        values: &[Value],
        message: Option<&str>,
    ) -> Self {
        let Some(value) = self.change(field) else {
            return self;
        };
        if !values
            .iter()
            .any(|candidate| values_equal(candidate, &value))
        {
            self.add_enum_error(field, message.unwrap_or("is invalid"), "inclusion", values);
        }
        self
    }

    /// `validate_exclusion/4`.
    pub fn validate_exclusion(
        mut self,
        field: &str,
        values: &[Value],
        message: Option<&str>,
    ) -> Self {
        let Some(value) = self.change(field) else {
            return self;
        };
        if values
            .iter()
            .any(|candidate| values_equal(candidate, &value))
        {
            self.add_enum_error(field, message.unwrap_or("is reserved"), "exclusion", values);
        }
        self
    }

    /// `validate_subset/4` over a list change.
    pub fn validate_subset(mut self, field: &str, values: &[Value], message: Option<&str>) -> Self {
        let Some(value) = self.change(field) else {
            return self;
        };
        let Some(items) = value.as_array() else {
            self.add_error(field, "validate_subset expects a list", "rust");
            return self;
        };
        let all_included = items
            .iter()
            .all(|item| values.iter().any(|candidate| values_equal(candidate, item)));
        if !all_included {
            self.add_enum_error(
                field,
                message.unwrap_or("has an invalid entry"),
                "subset",
                values,
            );
        }
        self
    }

    /// `validate_confirmation/3`: compares the raw `<field>` and
    /// `<field>_confirmation` params and reports on the confirmation field.
    pub fn validate_confirmation(
        mut self,
        field: &str,
        required: bool,
        message: Option<&str>,
    ) -> Self {
        let confirmation_field = format!("{field}_confirmation");
        let value = self.params.get(field).cloned().unwrap_or(Value::Null);
        match self.params.get(&confirmation_field) {
            Some(confirmation) if *confirmation == value => {}
            Some(_) => {
                let text = message.unwrap_or("does not match confirmation");
                self.add_error(&confirmation_field, text, "confirmation");
            }
            None if required => self.add_error(&confirmation_field, "can't be blank", "required"),
            None => {}
        }
        self
    }

    /// `validate_acceptance/3`: the raw param must cast to `true`.
    pub fn validate_acceptance(mut self, field: &str, message: Option<&str>) -> Self {
        let accepted = self
            .params
            .get(field)
            .and_then(|value| cast_value(&FieldType::Boolean, value))
            .is_some_and(|value| value == Value::Bool(true));
        if !accepted {
            self.add_error(field, message.unwrap_or("must be accepted"), "acceptance");
        }
        self
    }

    /// `Ecto.Changeset.add_error/4` with only the `validation` option.
    pub fn add_error(&mut self, field: &str, message: &str, validation: &str) {
        self.errors.push(Error {
            field: field.to_string(),
            message: message.to_string(),
            options: vec![("validation".to_string(), Value::from(validation))],
        });
    }

    fn add_enum_error(&mut self, field: &str, message: &str, validation: &str, values: &[Value]) {
        self.errors.push(Error {
            field: field.to_string(),
            message: message.to_string(),
            options: vec![
                ("validation".to_string(), Value::from(validation)),
                ("enum".to_string(), Value::Array(values.to_vec())),
            ],
        });
    }

    /// The change `validate_change/3` would run on: present and not null.
    fn change(&self, field: &str) -> Option<Value> {
        self.changes
            .get(field)
            .filter(|value| !value.is_null())
            .cloned()
    }
}

/// Ecto's default empty value is the empty string, checked after trimming
/// on every type but `:binary`: such a string is null, and such strings are
/// dropped from a list.
fn filter_empty(field_type: &FieldType, value: &Value) -> Value {
    match (field_type, value) {
        (FieldType::Binary, _) => value.clone(),
        (_, Value::String(text)) if text.trim().is_empty() => Value::Null,
        (FieldType::Array(inner), Value::Array(items)) => Value::Array(
            items
                .iter()
                .filter(|item| !filter_empty(inner, item).is_null())
                .cloned()
                .collect(),
        ),
        _ => value.clone(),
    }
}

/// `validate_required/3` treats null and the empty string as missing.
fn is_missing(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::String(text) => text.is_empty(),
        _ => false,
    }
}

static UUID: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$")
        .expect("uuid regex")
});
static DATE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\d{4}-\d{2}-\d{2}$").expect("date regex"));
static TIME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\d{2}:\d{2}:\d{2}(\.\d{1,6})?$").expect("time regex"));
static NAIVE_DATETIME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}(\.\d{1,6})?(Z|[+-]\d{2}:?\d{2})?$")
        .expect("naive datetime regex")
});
static UTC_DATETIME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}(\.\d{1,6})?(Z|[+-]\d{2}:?\d{2})$")
        .expect("utc datetime regex")
});

/// `Ecto.Type.cast/2` for the built-in types. `None` is Ecto's `:error`,
/// reported as "is invalid". Dates and times are checked for shape, not
/// for a valid calendar date.
fn cast_value(field_type: &FieldType, value: &Value) -> Option<Value> {
    if value.is_null() {
        return Some(Value::Null);
    }
    match field_type {
        FieldType::String | FieldType::Binary => value.is_string().then(|| value.clone()),
        FieldType::Integer | FieldType::Id => cast_integer(value),
        FieldType::Float | FieldType::Decimal => cast_float(value),
        FieldType::Boolean => cast_boolean(value),
        FieldType::Uuid => value
            .as_str()
            .filter(|text| UUID.is_match(text))
            .map(|text| Value::from(text.to_ascii_lowercase())),
        FieldType::Enum(values) => value
            .as_str()
            .filter(|text| values.iter().any(|candidate| candidate == text))
            .map(|_| value.clone()),
        FieldType::Array(inner) => {
            let items = value.as_array()?;
            let cast: Option<Vec<Value>> =
                items.iter().map(|item| cast_value(inner, item)).collect();
            cast.map(Value::Array)
        }
        FieldType::Map => value.is_object().then(|| value.clone()),
        FieldType::Date => cast_text(value, &DATE),
        FieldType::Time => cast_text(value, &TIME),
        FieldType::NaiveDatetime | FieldType::NaiveDatetimeUsec => {
            cast_text(value, &NAIVE_DATETIME)
        }
        FieldType::UtcDatetime | FieldType::UtcDatetimeUsec => cast_text(value, &UTC_DATETIME),
    }
}

fn cast_text(value: &Value, shape: &Regex) -> Option<Value> {
    value
        .as_str()
        .filter(|text| shape.is_match(text))
        .map(|_| value.clone())
}

fn cast_integer(value: &Value) -> Option<Value> {
    match value {
        Value::Number(number) if number.is_i64() || number.is_u64() => Some(value.clone()),
        Value::String(text) if text.len() < 32 => text.parse::<i64>().ok().map(Value::from),
        _ => None,
    }
}

fn cast_float(value: &Value) -> Option<Value> {
    match value {
        Value::Number(number) => number.as_f64().map(Value::from),
        Value::String(text) => text
            .parse::<f64>()
            .ok()
            .filter(|f| f.is_finite())
            .map(Value::from),
        _ => None,
    }
}

fn cast_boolean(value: &Value) -> Option<Value> {
    match value {
        Value::Bool(_) => Some(value.clone()),
        Value::String(text) if text == "true" || text == "1" => Some(Value::Bool(true)),
        Value::String(text) if text == "false" || text == "0" => Some(Value::Bool(false)),
        _ => None,
    }
}

/// Equality that treats `1` and `1.0` alike, as Ecto compares after casting.
fn values_equal(a: &Value, b: &Value) -> bool {
    match (a.as_f64(), b.as_f64()) {
        (Some(x), Some(y)) => x == y,
        _ => a == b,
    }
}

fn as_f64(number: &serde_json::Number) -> f64 {
    number.as_f64().unwrap_or(f64::NAN)
}
