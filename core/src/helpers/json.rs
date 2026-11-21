//! Transforms of JSON values.

use serde_json::{Map, Value};

/// The fields of a JSON object, or none for anything else.
pub fn object_fields(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(fields) => fields,
        _ => Map::new(),
    }
}
