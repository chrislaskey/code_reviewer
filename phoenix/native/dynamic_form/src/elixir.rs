//! The few Elixir functions the library's behavior rests on, over JSON
//! values: `to_string/1`, truthiness, `String.capitalize/1`,
//! `Integer.parse/1` and `Float.parse/1`, and Phoenix's HTML escaping.

use serde_json::Value;

/// `to_string/1` of a value the library shows: a string as is, `nil` as
/// `""`, a float as Elixir writes it, `1.0` or `1.0e20`. A list or map,
/// which Elixir can't turn into a string, is written as JSON.
pub fn to_string(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(text) => text.clone(),
        Value::Bool(flag) => flag.to_string(),
        Value::Number(number) => match (number.as_i64(), number.as_u64()) {
            (Some(integer), _) => integer.to_string(),
            (_, Some(integer)) => integer.to_string(),
            _ => float_to_string(number.as_f64().unwrap_or_default()),
        },
        other => other.to_string(),
    }
}

/// Elixir's `Float.to_string/1`: the shortest digits that read back as
/// the same float, in plain notation or scientific, whichever is shorter,
/// with at least one digit after the point.
pub fn float_to_string(float: f64) -> String {
    // Rust's `{:e}` gives the shortest round-trip digits, like `1.5e-7`
    let scientific = format!("{float:e}");
    let (mantissa, exponent) = scientific.split_once('e').unwrap_or((&scientific, "0"));
    let exponent: i32 = exponent.parse().unwrap_or(0);
    let negative = mantissa.starts_with('-');
    let digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();

    let sign = if negative { "-" } else { "" };
    let fraction = if digits.len() > 1 { &digits[1..] } else { "0" };
    let scientific = format!("{sign}{}.{fraction}e{exponent}", &digits[..1]);

    let point = exponent + 1;
    let plain = if point <= 0 {
        format!("{sign}0.{}{digits}", "0".repeat((-point) as usize))
    } else if point as usize >= digits.len() {
        format!(
            "{sign}{digits}{}.0",
            "0".repeat(point as usize - digits.len())
        )
    } else {
        let (whole, rest) = digits.split_at(point as usize);
        format!("{sign}{whole}.{rest}")
    };

    if scientific.len() < plain.len() {
        scientific
    } else {
        plain
    }
}

/// Elixir truthiness: anything but `nil` and `false`.
pub fn truthy(value: &Value) -> bool {
    !matches!(value, Value::Null | Value::Bool(false))
}

/// `DynamicForm.Instance.blank?/1`: `nil`, `false` or `""`.
pub fn blank(value: &Value) -> bool {
    match value {
        Value::Null | Value::Bool(false) => true,
        Value::String(text) => text.is_empty(),
        _ => false,
    }
}

/// `String.capitalize/1`: the first letter upper case, the rest lower.
pub fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first
            .to_uppercase()
            .chain(chars.flat_map(char::to_lowercase))
            .collect(),
        None => String::new(),
    }
}

/// `Phoenix.HTML.html_escape/1`.
pub fn html_escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            c => escaped.push(c),
        }
    }
    escaped
}

/// A number from text the way the library reads one: `Integer.parse/1`
/// taking the whole text, else `Float.parse/1` taking it.
pub fn parse_number(text: &str) -> Option<Value> {
    if let Some(integer) = parse_integer(text) {
        return Some(Value::from(integer));
    }
    parse_float(text).and_then(|float| serde_json::Number::from_f64(float).map(Value::Number))
}

/// `Integer.parse/1` when it takes the whole text: a sign and digits.
fn parse_integer(text: &str) -> Option<i64> {
    let digits = text.strip_prefix(['+', '-']).unwrap_or(text);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let digits = digits.trim_start_matches('0');
    let value: i64 = if digits.is_empty() {
        0
    } else {
        digits.parse().ok()?
    };
    Some(if text.starts_with('-') { -value } else { value })
}

/// `Float.parse/1` when it takes the whole text: a sign, digits, then
/// optionally a point and digits, then optionally an exponent.
fn parse_float(text: &str) -> Option<f64> {
    let bytes = text.as_bytes();
    let mut at = 0;
    if matches!(bytes.first(), Some(b'+' | b'-')) {
        at += 1;
    }
    let digits = |at: &mut usize| {
        let start = *at;
        while bytes.get(*at).is_some_and(u8::is_ascii_digit) {
            *at += 1;
        }
        *at > start
    };
    if !digits(&mut at) {
        return None;
    }
    if bytes.get(at) == Some(&b'.') {
        let mut after = at + 1;
        if !digits(&mut after) {
            return None;
        }
        at = after;
    }
    if matches!(bytes.get(at), Some(b'e' | b'E')) {
        let mut after = at + 1;
        if matches!(bytes.get(after), Some(b'+' | b'-')) {
            after += 1;
        }
        if !digits(&mut after) {
            return None;
        }
        at = after;
    }
    if at != bytes.len() {
        return None;
    }
    text.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floats_read_as_elixir_writes_them() {
        assert_eq!(float_to_string(1.0), "1.0");
        assert_eq!(float_to_string(0.1), "0.1");
        assert_eq!(float_to_string(100.0), "100.0");
        assert_eq!(float_to_string(1.0e20), "1.0e20");
        assert_eq!(float_to_string(1.5e-7), "1.5e-7");
        assert_eq!(float_to_string(-2.5), "-2.5");
    }

    #[test]
    fn numbers_parse_as_integer_and_float_parse_do() {
        let parsed = |text| parse_number(text).map(|value| value.to_string());
        assert_eq!(parsed("07").as_deref(), Some("7"));
        assert_eq!(parsed("+5").as_deref(), Some("5"));
        assert_eq!(parsed("-5").as_deref(), Some("-5"));
        assert_eq!(parsed("1e3").as_deref(), Some("1000.0"));
        assert_eq!(parsed("1.5E2").as_deref(), Some("150.0"));
        assert_eq!(parsed("1.0").as_deref(), Some("1.0"));
        for text in ["1.", ".5", "5 ", "1_000", "inf", "1e", "12abc", ""] {
            assert_eq!(parsed(text), None, "parsed {text:?}");
        }
    }

    #[test]
    fn html_escapes_as_phoenix_does() {
        assert_eq!(
            html_escape(r#"<a href="x">&'</a>"#),
            "&lt;a href=&quot;x&quot;&gt;&amp;&#39;&lt;/a&gt;"
        );
    }
}
