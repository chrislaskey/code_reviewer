//! Turns the entries of a submitted form into the params the Phoenix app
//! sees, with the bracket rules of `Plug.Conn.Query`: `tags[]` collects a
//! list, `post[body]` nests a map, a key given twice keeps the last
//! value. This is the step before `Changeset::cast`, and it has to be the
//! same in every app, or the same form would reach the same rules as
//! different params. A desktop or mobile app passes its `FormData` entries
//! here instead of decoding them itself.
//!
//! The port follows Plug's decoder entry for entry: `users[foo][bar]` is
//! the levels `users`, `users[foo]` and `users[foo][bar]`, each holding its
//! entries in order, and a level whose last entry has no key is a list.

use std::collections::HashMap;

use serde_json::{Map, Value};

use crate::Params;

/// `Plug.Conn.Query`'s limit on nested brackets.
const MAX_NESTING: usize = 32;

/// An entry under a level: a value, or a pointer to the level below it.
#[derive(Debug, Clone)]
enum Entry {
    Value(String),
    Pointer(String),
}

/// The params for the `(name, value)` entries of a form, in the order the
/// form lists them.
pub fn decode<'a, I>(entries: I) -> Result<Params, String>
where
    I: IntoIterator<Item = (&'a str, &'a str)>,
{
    let mut levels: HashMap<String, Vec<(Option<String>, Entry)>> = HashMap::new();
    levels.insert(ROOT.to_string(), Vec::new());

    for (name, value) in entries {
        let keys = split_keys(name)?;
        insert_keys(&keys, Entry::Value(value.to_string()), &mut levels);
    }

    Ok(finalize_map(ROOT, &levels))
}

/// The params for a query string or a URL-encoded form body.
pub fn decode_query(query: &str) -> Result<Params, String> {
    let pairs: Vec<(String, String)> = query
        .split('&')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let (key, value) = part.split_once('=').unwrap_or((part, ""));
            (unescape(key), unescape(value))
        })
        .collect();
    decode(
        pairs
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str())),
    )
}

const ROOT: &str = "\u{0}root";

/// The levels a name addresses, from the root down: `users[foo][bar]` is
/// `[(root, "users"), ("users", "foo"), ("users[foo]", "bar")]`. A name
/// without brackets, or with brackets it does not end in, is one key.
fn split_keys(name: &str) -> Result<Vec<(String, Option<String>)>, String> {
    let Some(open) = name.find('[') else {
        return Ok(vec![(ROOT.to_string(), Some(name.to_string()))]);
    };
    if open == 0 || !name.ends_with(']') {
        return Ok(vec![(ROOT.to_string(), Some(name.to_string()))]);
    }

    let mut keys = vec![(ROOT.to_string(), Some(name[..open].to_string()))];
    let mut level_end = open;
    let mut rest = &name[open + 1..name.len() - 1];

    loop {
        if keys.len() > MAX_NESTING {
            return Err(format!(
                "maximum query nesting is {MAX_NESTING}, got a query with {} keys",
                keys.len()
            ));
        }
        let level = name[..level_end].to_string();
        match rest.find("][") {
            Some(close) => {
                let key = &rest[..close];
                keys.push((level, non_empty(key)));
                level_end += close + 2;
                rest = &rest[close + 2..];
            }
            None => {
                keys.push((level, non_empty(rest)));
                return Ok(keys);
            }
        }
    }
}

fn non_empty(key: &str) -> Option<String> {
    if key.is_empty() {
        None
    } else {
        Some(key.to_string())
    }
}

/// Adds the entry under the deepest level, creating the levels above it
/// with pointers down, and stopping at the first level that exists.
fn insert_keys(
    keys: &[(String, Option<String>)],
    value: Entry,
    levels: &mut HashMap<String, Vec<(Option<String>, Entry)>>,
) {
    let mut value = value;
    for (level, key) in keys.iter().rev() {
        if let Some(entries) = levels.get_mut(level) {
            entries.push((key.clone(), value));
            return;
        }
        levels.insert(level.clone(), vec![(key.clone(), value)]);
        value = Entry::Pointer(level.clone());
    }
}

fn finalize_pointer(level: &str, levels: &HashMap<String, Vec<(Option<String>, Entry)>>) -> Value {
    match levels.get(level).and_then(|entries| entries.last()) {
        Some((None, _)) => Value::Array(finalize_list(level, levels)),
        _ => Value::Object(finalize_map(level, levels)),
    }
}

/// A level as a map: entries without a key are dropped, and a key given
/// twice keeps its last value.
fn finalize_map(
    level: &str,
    levels: &HashMap<String, Vec<(Option<String>, Entry)>>,
) -> Map<String, Value> {
    let mut map = Map::new();
    for (key, entry) in levels.get(level).into_iter().flatten() {
        let Some(key) = key else { continue };
        map.insert(key.clone(), finalize_entry(entry, levels));
    }
    map
}

/// A level as a list: entries with a key are dropped.
fn finalize_list(
    level: &str,
    levels: &HashMap<String, Vec<(Option<String>, Entry)>>,
) -> Vec<Value> {
    levels
        .get(level)
        .into_iter()
        .flatten()
        .filter(|(key, _)| key.is_none())
        .map(|(_, entry)| finalize_entry(entry, levels))
        .collect()
}

fn finalize_entry(entry: &Entry, levels: &HashMap<String, Vec<(Option<String>, Entry)>>) -> Value {
    match entry {
        Entry::Value(value) => Value::String(value.clone()),
        Entry::Pointer(level) => finalize_pointer(level, levels),
    }
}

/// `x-www-form-urlencoded`: `+` is a space and `%XX` a byte.
fn unescape(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => {
                match std::str::from_utf8(&bytes[i + 1..i + 3])
                    .ok()
                    .and_then(|hex| u8::from_str_radix(hex, 16).ok())
                {
                    Some(byte) => {
                        out.push(byte);
                        i += 2;
                    }
                    None => out.push(b'%'),
                }
            }
            byte => out.push(byte),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}
