//! The form decoder gives the params `Plug.Conn.Query` gives: every
//! expectation here is what Plug returned for the same entries.

use ecto_changeset::form;
use serde_json::{Value, json};

fn decode(entries: &[(&str, &str)]) -> Value {
    Value::Object(form::decode(entries.iter().copied()).expect("decodes"))
}

#[test]
fn plain_keys() {
    assert_eq!(
        decode(&[("name", "Ada"), ("email", "ada@example.com")]),
        json!({"email": "ada@example.com", "name": "Ada"})
    );
}

#[test]
fn empty_brackets_collect_a_list_in_order() {
    assert_eq!(
        decode(&[("tags[]", "vip"), ("tags[]", "partner")]),
        json!({"tags": ["vip", "partner"]})
    );
}

#[test]
fn brackets_nest_maps_and_lists() {
    assert_eq!(
        decode(&[
            ("contact[name]", "Ada"),
            ("contact[tags][]", "vip"),
            ("contact[tags][]", "partner")
        ]),
        json!({"contact": {"name": "Ada", "tags": ["vip", "partner"]}})
    );
    assert_eq!(
        decode(&[("a[b][c]", "1"), ("a[b][d]", "2"), ("a[e]", "3")]),
        json!({"a": {"b": {"c": "1", "d": "2"}, "e": "3"}})
    );
}

#[test]
fn a_key_given_twice_keeps_the_last_value() {
    assert_eq!(
        decode(&[("name", "first"), ("name", "last")]),
        json!({"name": "last"})
    );
    assert_eq!(
        decode(&[("tags[]", "vip"), ("tags", "flat")]),
        json!({"tags": "flat"})
    );
    assert_eq!(
        decode(&[("tags", "flat"), ("tags[]", "vip")]),
        json!({"tags": ["vip"]})
    );
}

#[test]
fn list_entries_with_keys_share_one_map() {
    assert_eq!(
        decode(&[("people[][name]", "Ada"), ("people[][age]", "36")]),
        json!({"people": [{"age": "36", "name": "Ada"}]})
    );
}

#[test]
fn the_last_entry_decides_between_list_and_map() {
    assert_eq!(
        decode(&[("tags[]", "vip"), ("tags[x]", "ignored")]),
        json!({"tags": {"x": "ignored"}})
    );
}

#[test]
fn malformed_brackets_are_plain_keys() {
    assert_eq!(
        decode(&[("odd[", "1"), ("[name]", "2"), ("", "3")]),
        json!({"": "3", "[name]": "2", "odd[": "1"})
    );
}

#[test]
fn empty_values_are_kept() {
    assert_eq!(
        decode(&[("name", ""), ("tags[]", "")]),
        json!({"name": "", "tags": [""]})
    );
}

#[test]
fn too_deep_nesting_is_an_error() {
    let name = format!("a{}", "[b]".repeat(33));
    assert!(form::decode([(name.as_str(), "1")]).is_err());
}

#[test]
fn a_query_string_is_unescaped_first() {
    let params =
        form::decode_query("name=Ada+Lovelace&tags%5B%5D=vip&note=%C3%A9%26x").expect("decodes");
    assert_eq!(
        Value::Object(params),
        json!({"name": "Ada Lovelace", "note": "é&x", "tags": ["vip"]})
    );
}
