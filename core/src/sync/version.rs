//! JSON encoding for global sync versions. Decimal strings preserve 64-bit
//! values when a row or cursor passes through JavaScript.

use serde::{Deserialize, Deserializer, Serializer, de};

/// Writes a global sync version as a decimal string.
pub fn serialize<S: Serializer>(version: &i64, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.collect_str(version)
}

/// Reads a nonnegative global sync version.
pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<i64, D::Error> {
    parse(&String::deserialize(deserializer)?)
}

pub(crate) fn parse<E: de::Error>(value: &str) -> Result<i64, E> {
    let version = value.parse::<i64>().map_err(E::custom)?;
    if version < 0 {
        return Err(E::custom("global sync version must be nonnegative"));
    }
    Ok(version)
}

/// Serde adapter for a row that may have been created offline.
pub mod optional {
    use super::*;
    use serde::Serialize;

    /// Serializes a version as a decimal string, or null before the row syncs.
    pub fn serialize<S: Serializer>(
        version: &Option<i64>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        version.map(|value| value.to_string()).serialize(serializer)
    }

    /// Reads a decimal string or null.
    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<i64>, D::Error> {
        Option::<String>::deserialize(deserializer)?
            .as_deref()
            .map(parse)
            .transpose()
    }
}
