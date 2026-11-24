//! The cast and each validation against what `Ecto.Changeset` returns for
//! the same data and params: every expectation here is what Ecto said.

use std::collections::BTreeMap;

use ecto_changeset::{Changeset, Count, Error, FieldType, Length, NumberCheck, Params};
use serde_json::{Value, json};

fn params(value: Value) -> Params {
    value.as_object().expect("an object").clone()
}

fn types() -> BTreeMap<String, FieldType> {
    BTreeMap::from([
        ("name".to_string(), FieldType::String),
        ("email".to_string(), FieldType::String),
        ("age".to_string(), FieldType::Integer),
        ("score".to_string(), FieldType::Float),
        ("status".to_string(), FieldType::String),
        (
            "tags".to_string(),
            FieldType::Array(Box::new(FieldType::String)),
        ),
        ("notes".to_string(), FieldType::String),
        ("terms".to_string(), FieldType::Boolean),
        ("owner_id".to_string(), FieldType::Uuid),
        (
            "role".to_string(),
            FieldType::Enum(vec!["member".to_string(), "admin".to_string()]),
        ),
        ("born_on".to_string(), FieldType::Date),
        ("at".to_string(), FieldType::UtcDatetimeUsec),
        ("raw".to_string(), FieldType::Binary),
    ])
}

/// The example `App.Data.Contact.changeset/2` of the spike: every rule once.
fn contact(data: Value, params_value: Value) -> Changeset {
    Changeset::cast(
        &params(data),
        &types(),
        &params(params_value),
        &["name", "email", "age", "status", "tags", "notes"],
    )
    .validate_required(&["name", "email"])
    .validate_length("name", &Length::default().min(2).max(100), None)
    .validate_format(
        "email",
        r"^[^\s@]+@[^\s@]+$",
        Some("must look like an email address"),
    )
    .validate_length("email", &Length::default().max(160), None)
    .validate_number(
        "age",
        &[
            NumberCheck::GreaterThanOrEqualTo(0.into()),
            NumberCheck::LessThan(150.into()),
        ],
        None,
    )
    .validate_inclusion("status", &[json!("lead"), json!("customer")], None)
    .validate_subset("tags", &[json!("vip"), json!("partner")], None)
    .validate_length("tags", &Length::default().max(3), None)
}

fn valid() -> Value {
    json!({"name": "Ada Lovelace", "email": "ada@example.com", "age": "36", "status": "customer", "tags": ["vip"]})
}

fn errors(changeset: &Changeset) -> Vec<(String, String, Value)> {
    changeset
        .errors()
        .iter()
        .map(|error| {
            (
                error.field.clone(),
                error.message.clone(),
                json!(error.options),
            )
        })
        .collect()
}

fn error(field: &str, message: &str, options: Value) -> (String, String, Value) {
    (field.to_string(), message.to_string(), options)
}

#[test]
fn valid_params_give_typed_changes_and_no_errors() {
    let changeset = contact(json!({}), valid());
    assert!(changeset.valid());
    assert_eq!(changeset.changes()["age"], json!(36));
    assert_eq!(changeset.changes()["tags"], json!(["vip"]));
    assert_eq!(changeset.required(), ["name", "email"]);
}

#[test]
fn required_fields_report_as_ecto_does() {
    let changeset = contact(json!({}), json!({}));
    assert_eq!(
        errors(&changeset),
        vec![
            error(
                "name",
                "can't be blank",
                json!([["validation", "required"]])
            ),
            error(
                "email",
                "can't be blank",
                json!([["validation", "required"]])
            ),
        ]
    );
}

#[test]
fn a_string_blank_once_trimmed_is_empty() {
    let changeset = contact(json!({}), json!({"name": "  ", "email": ""}));
    assert_eq!(changeset.errors().len(), 2);
    assert!(changeset.changes().is_empty());
}

#[test]
fn a_binary_field_keeps_a_blank_string() {
    let changeset = Changeset::cast(
        &params(json!({})),
        &types(),
        &params(json!({"raw": " "})),
        &["raw"],
    );
    assert_eq!(changeset.changes()["raw"], json!(" "));
}

#[test]
fn empty_strings_are_dropped_from_a_list() {
    let changeset = contact(json!({}), json!({"tags": ["", "vip", "  "]}));
    assert_eq!(changeset.changes()["tags"], json!(["vip"]));
    let changeset = contact(json!({"tags": []}), json!({"tags": [""]}));
    assert!(!changeset.changes().contains_key("tags"));
}

#[test]
fn length_counts_graphemes_and_carries_ectos_options() {
    let too_short = contact(json!({}), json!({"name": "A", "email": "a@b"}));
    assert_eq!(
        errors(&too_short),
        vec![error(
            "name",
            "should be at least %{count} character(s)",
            json!([
                ["count", 2],
                ["validation", "length"],
                ["kind", "min"],
                ["type", "string"]
            ])
        )]
    );
    assert_eq!(
        too_short.errors()[0].text(),
        "should be at least 2 character(s)"
    );

    let one_grapheme = contact(json!({}), json!({"name": "e\u{301}", "email": "a@b"}));
    assert_eq!(one_grapheme.errors().len(), 1);

    let too_long = contact(json!({}), json!({"name": "a".repeat(101), "email": "a@b"}));
    assert_eq!(too_long.errors()[0].option("kind"), Some(&json!("max")));
    assert_eq!(too_long.errors()[0].option("count"), Some(&json!(100)));
}

#[test]
fn length_counts_bytes_items_and_exact_lengths() {
    let bytes = Changeset::cast(
        &params(json!({})),
        &types(),
        &params(json!({"name": "é".repeat(40)})),
        &["name"],
    )
    .validate_length("name", &Length::default().max(72).count(Count::Bytes), None);
    assert_eq!(
        errors(&bytes),
        vec![error(
            "name",
            "should be at most %{count} byte(s)",
            json!([
                ["count", 72],
                ["validation", "length"],
                ["kind", "max"],
                ["type", "binary"]
            ])
        )]
    );

    let items = contact(json!({}), json!({"tags": ["vip", "partner", "vip", "vip"]}));
    assert_eq!(
        errors(&items)[2],
        error(
            "tags",
            "should have at most %{count} item(s)",
            json!([
                ["count", 3],
                ["validation", "length"],
                ["kind", "max"],
                ["type", "list"]
            ])
        )
    );

    let exact = Changeset::cast(
        &params(json!({})),
        &types(),
        &params(json!({"name": "abc"})),
        &["name"],
    )
    .validate_length("name", &Length::default().is(2), Some("needs %{count}"));
    assert_eq!(exact.errors()[0].message, "needs %{count}");
    assert_eq!(exact.errors()[0].option("kind"), Some(&json!("is")));
}

#[test]
fn format_uses_the_message_given_and_runs_before_length() {
    let changeset = contact(json!({}), json!({"name": "Ada", "email": "a".repeat(161)}));
    assert_eq!(
        errors(&changeset),
        vec![
            error(
                "email",
                "must look like an email address",
                json!([["validation", "format"]])
            ),
            error(
                "email",
                "should be at most %{count} character(s)",
                json!([
                    ["count", 160],
                    ["validation", "length"],
                    ["kind", "max"],
                    ["type", "string"]
                ])
            ),
        ]
    );
}

#[test]
fn format_flags_are_inline_and_a_bad_pattern_reports() {
    let ok = Changeset::cast(
        &params(json!({})),
        &types(),
        &params(json!({"name": "HTTP://x"})),
        &["name"],
    )
    .validate_format("name", r"(?i)^https?://", None);
    assert!(ok.valid());

    let bad = Changeset::cast(
        &params(json!({})),
        &types(),
        &params(json!({"name": "x"})),
        &["name"],
    )
    .validate_format("name", r"(?<=a)b", None);
    assert_eq!(bad.errors()[0].validation(), "rust");
}

#[test]
fn a_cast_error_names_the_type_and_stops_later_validations() {
    let changeset = contact(
        json!({}),
        json!({"name": "Ada", "email": "a@b", "age": "old", "tags": "vip"}),
    );
    assert_eq!(
        errors(&changeset),
        vec![
            error(
                "age",
                "is invalid",
                json!([["type", "integer"], ["validation", "cast"]])
            ),
            error(
                "tags",
                "is invalid",
                json!([["type", {"array": "string"}], ["validation", "cast"]])
            ),
        ]
    );
}

#[test]
fn number_reports_the_first_failing_check_with_ectos_options() {
    let negative = contact(json!({}), json!({"name": "Ada", "email": "a@b", "age": -1}));
    assert_eq!(
        errors(&negative),
        vec![error(
            "age",
            "must be greater than or equal to %{number}",
            json!([
                ["validation", "number"],
                ["kind", "greater_than_or_equal_to"],
                ["number", 0]
            ])
        )]
    );
    let high = contact(
        json!({}),
        json!({"name": "Ada", "email": "a@b", "age": "150"}),
    );
    assert_eq!(high.errors()[0].option("kind"), Some(&json!("less_than")));
    assert_eq!(high.errors()[0].text(), "must be less than 150");
    assert!(
        contact(
            json!({}),
            json!({"name": "Ada", "email": "a@b", "age": 149})
        )
        .valid()
    );
}

#[test]
fn inclusion_exclusion_and_subset_carry_the_enum() {
    let status = contact(
        json!({}),
        json!({"name": "Ada", "email": "a@b", "status": "prospect"}),
    );
    assert_eq!(
        errors(&status),
        vec![error(
            "status",
            "is invalid",
            json!([["validation", "inclusion"], ["enum", ["lead", "customer"]]])
        )]
    );

    let tags = contact(
        json!({}),
        json!({"name": "Ada", "email": "a@b", "tags": ["vip", "spam"]}),
    );
    assert_eq!(
        errors(&tags),
        vec![error(
            "tags",
            "has an invalid entry",
            json!([["validation", "subset"], ["enum", ["vip", "partner"]]])
        )]
    );

    let reserved = Changeset::cast(
        &params(json!({})),
        &types(),
        &params(json!({"name": "admin"})),
        &["name"],
    )
    .validate_exclusion("name", &[json!("admin")], None);
    assert_eq!(
        errors(&reserved),
        vec![error(
            "name",
            "is reserved",
            json!([["validation", "exclusion"], ["enum", ["admin"]]])
        )]
    );
}

#[test]
fn confirmation_and_acceptance_read_the_raw_params() {
    let signup = |extra: Value| {
        let mut all = valid();
        all.as_object_mut().unwrap().extend(params(extra));
        contact(json!({}), all)
            .validate_confirmation("email", true, None)
            .validate_acceptance("terms", None)
    };

    assert!(signup(json!({"email_confirmation": "ada@example.com", "terms": "true"})).valid());
    assert_eq!(
        errors(&signup(
            json!({"email_confirmation": "other@example.com", "terms": "1"})
        )),
        vec![error(
            "email_confirmation",
            "does not match confirmation",
            json!([["validation", "confirmation"]])
        )]
    );
    assert_eq!(
        errors(&signup(json!({"terms": "true"}))),
        vec![error(
            "email_confirmation",
            "can't be blank",
            json!([["validation", "required"]])
        )]
    );
    assert_eq!(
        errors(&signup(
            json!({"email_confirmation": "ada@example.com", "terms": "false"})
        )),
        vec![error(
            "terms",
            "must be accepted",
            json!([["validation", "acceptance"]])
        )]
    );
    assert_eq!(
        signup(json!({"email_confirmation": "ada@example.com"})).errors()[0].validation(),
        "acceptance"
    );
    let optional = contact(json!({}), valid()).validate_confirmation("email", false, None);
    assert!(optional.valid());
}

#[test]
fn only_changes_are_validated() {
    assert!(contact(json!({"name": "A", "email": "bad"}), json!({})).valid());

    let one = contact(
        json!({"name": "Ada", "email": "ada@example.com"}),
        json!({"name": "B"}),
    );
    assert_eq!(one.errors().len(), 1);
    assert_eq!(one.errors()[0].field, "name");

    let same = contact(
        json!({"name": "A"}),
        json!({"name": "A", "email": "ada@example.com"}),
    );
    assert!(same.valid());
    assert!(!same.changes().contains_key("name"));
}

#[test]
fn a_required_field_set_by_the_app_is_read_from_data() {
    let changeset = Changeset::cast(
        &params(json!({"owner_id": "0199f3a0-6c1e-7c3e-8f1a-2b3c4d5e6f70"})),
        &types(),
        &params(json!({"name": "Ada"})),
        &["name"],
    )
    .validate_required(&["owner_id", "name"]);
    assert!(changeset.valid());
    assert_eq!(
        changeset.get_field("owner_id"),
        json!("0199f3a0-6c1e-7c3e-8f1a-2b3c4d5e6f70")
    );
}

#[test]
fn casts_uuids_enums_booleans_floats_and_dates_as_ecto_does() {
    let cast = |field: &str, value: Value| {
        let changeset = Changeset::cast(
            &params(json!({})),
            &types(),
            &params(json!({field: value})),
            &[field],
        );
        changeset
            .changes()
            .get(field)
            .cloned()
            .ok_or_else(|| changeset.errors()[0].clone())
    };
    let invalid = |error: Result<Value, Error>| error.unwrap_err().validation() == "cast";

    assert_eq!(
        cast("owner_id", json!("0199F3A0-6C1E-7C3E-8F1A-2B3C4D5E6F70")).unwrap(),
        json!("0199f3a0-6c1e-7c3e-8f1a-2b3c4d5e6f70")
    );
    assert!(invalid(cast("owner_id", json!("not-a-uuid"))));
    assert_eq!(cast("role", json!("admin")).unwrap(), json!("admin"));
    assert!(invalid(cast("role", json!("owner"))));
    assert_eq!(cast("terms", json!("1")).unwrap(), json!(true));
    assert_eq!(cast("terms", json!("false")).unwrap(), json!(false));
    assert!(invalid(cast("terms", json!("yes"))));
    assert_eq!(cast("score", json!("1.5")).unwrap(), json!(1.5));
    assert!(invalid(cast("score", json!("fast"))));
    assert_eq!(
        cast("born_on", json!("2026-10-26")).unwrap(),
        json!("2026-10-26")
    );
    assert!(invalid(cast("born_on", json!("26/10/2026"))));
    assert_eq!(
        cast("at", json!("2026-10-26T10:00:00.123456Z")).unwrap(),
        json!("2026-10-26T10:00:00.123456Z")
    );
    assert!(invalid(cast("at", json!("2026-10-26T10:00:00"))));
}

#[test]
fn text_fills_in_the_options() {
    let error = Error {
        field: "age".to_string(),
        message: "must be less than %{number}".to_string(),
        options: vec![("number".to_string(), json!(150))],
    };
    assert_eq!(error.text(), "must be less than 150");
}

/// `Ecto.Changeset.cast/4` over `{%{name: "Ada", age: 3}, types}`, the
/// data the change functions below compare with.
fn ada(params_value: Value) -> Changeset {
    Changeset::cast(
        &params(json!({"name": "Ada", "age": 3})),
        &types(),
        &params(params_value),
        &["name", "tags", "age"],
    )
}

#[test]
fn params_are_the_raw_params_cast_from() {
    let changeset = ada(json!({"name": "Bea", "tags": ["a", ""], "extra": "x"}));

    assert_eq!(
        changeset.params(),
        &params(json!({"name": "Bea", "tags": ["a", ""], "extra": "x"}))
    );
    assert_eq!(changeset.changes()["tags"], json!(["a"]));
}

#[test]
fn put_change_stores_a_change_unless_it_equals_the_data() {
    let mut changeset = ada(json!({"name": "Bea", "tags": ["a", ""]}));

    changeset.put_change("tags", json!(["b"]));
    assert_eq!(changeset.changes()["tags"], json!(["b"]));

    changeset.put_change("name", json!("Ada"));
    assert!(!changeset.changes().contains_key("name"));

    // Ecto's integer equality takes 3.0 for 3
    changeset.put_change("age", json!(3.0));
    assert!(!changeset.changes().contains_key("age"));

    changeset.put_change("tags", json!([]));
    assert_eq!(changeset.changes()["tags"], json!([]));
}

#[test]
fn delete_change_falls_back_to_the_data() {
    let mut with_data = ada(json!({"name": "Bea"}));
    with_data.delete_change("name");
    with_data.delete_change("age");
    assert!(with_data.changes().is_empty());
    assert_eq!(with_data.get_field("name"), json!("Ada"));
    assert!(with_data.validate_required(&["name"]).valid());

    let mut without_data = Changeset::cast(
        &Params::new(),
        &types(),
        &params(json!({"name": "Bea"})),
        &["name"],
    );
    without_data.delete_change("name");
    assert_eq!(
        errors(&without_data.validate_required(&["name"])),
        [error(
            "name",
            "can't be blank",
            json!([["validation", "required"]])
        )]
    );
}
