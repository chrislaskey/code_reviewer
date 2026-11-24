//! `DynamicForm.Visibility`: the SurveyJS conditions of `visibleIf`,
//! `enableIf` and `requiredIf`, tokenized, parsed and evaluated as the
//! library does, with its loose equality. An expression that doesn't
//! parse is false.
//!
//! The library looks a name up with `Map.get(params, name) ||
//! Map.get(params, atom)`, over two kinds of map: the raw params, keyed
//! by strings, when it validates, and the cast changes, keyed by atoms,
//! when it renders. The two differ for `false`: over string keys `false`
//! falls through to the atom lookup and reads as `nil`. `Keys` says which
//! a scope is.

use serde_json::{Map, Value};

use crate::elixir;

/// How the map a condition reads was keyed in the library.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keys {
    /// Raw params, or a map whose keys were stringified: `false` reads as
    /// `nil`.
    Strings,
    /// A changeset's changes: values read as they are.
    Atoms,
}

/// The values a condition reads, and how they were keyed.
#[derive(Debug, Clone, Copy)]
pub struct Scope<'a> {
    pub values: &'a Map<String, Value>,
    pub keys: Keys,
}

impl<'a> Scope<'a> {
    pub fn new(values: &'a Map<String, Value>, keys: Keys) -> Self {
        Scope { values, keys }
    }

    /// `Visibility.get_field_value/2`.
    fn get(&self, name: &str) -> Value {
        if let Some(field) = name.strip_prefix("panel.") {
            // `Map.get(params, "panel.x")` reads string keys only
            return match (self.keys, self.values.get(name)) {
                (Keys::Strings, Some(value)) if !value.is_null() => value.clone(),
                _ => self.get(field),
            };
        }
        match (self.keys, self.values.get(name)) {
            (_, None) | (_, Some(Value::Null)) => Value::Null,
            (Keys::Strings, Some(Value::Bool(false))) => Value::Null,
            (_, Some(value)) => value.clone(),
        }
    }
}

/// `Visibility.condition_met?/3`: no condition, or `""`, gives `default`.
pub fn condition_met(expression: Option<&str>, scope: Scope, default: bool) -> bool {
    match expression {
        None | Some("") => default,
        Some(expression) => evaluate(expression, scope),
    }
}

/// `Visibility.evaluate_expression/2`.
pub fn evaluate(expression: &str, scope: Scope) -> bool {
    match tokenize(expression).and_then(|tokens| parse(&tokens)) {
        Ok(ast) => eval(&ast, scope),
        Err(_) => false,
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Field(String),
    Literal(Value),
    LParen,
    RParen,
    LBracket,
    RBracket,
    Comma,
    And,
    Or,
    Op(Op),
    Error(String),
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Op {
    Eq,
    Ne,
    Gt,
    Lt,
    Ge,
    Le,
    Empty,
    NotEmpty,
    Contains,
    NotContains,
    AnyOf,
    AllOf,
    NoneOf,
}

#[derive(Debug, Clone, PartialEq)]
enum Ast {
    Or(Box<Ast>, Box<Ast>),
    And(Box<Ast>, Box<Ast>),
    Unary(Op, Operand),
    Binary(Op, Operand, Operand),
}

#[derive(Debug, Clone, PartialEq)]
enum Operand {
    Field(String),
    Literal(Value),
}

fn tokenize(expression: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let mut rest = expression;

    while let Some(c) = rest.chars().next() {
        if matches!(c, ' ' | '\t' | '\n' | '\r') {
            rest = &rest[1..];
            continue;
        }
        if let Some(after) = rest.strip_prefix('{') {
            match after.split_once('}') {
                Some((name, after)) if !name.is_empty() => {
                    tokens.push(Token::Field(name.trim().to_string()));
                    rest = after;
                }
                _ => return Err("unterminated field reference".into()),
            }
            continue;
        }
        if c == '\'' || c == '"' {
            match rest[1..].split_once(c) {
                Some((value, after)) => {
                    tokens.push(Token::Literal(Value::String(value.to_string())));
                    rest = after;
                }
                None => return Err("unterminated string literal".into()),
            }
            continue;
        }

        let symbols: [(&str, Token); 13] = [
            ("(", Token::LParen),
            (")", Token::RParen),
            ("[", Token::LBracket),
            ("]", Token::RBracket),
            (",", Token::Comma),
            ("<=", Token::Op(Op::Le)),
            (">=", Token::Op(Op::Ge)),
            ("<>", Token::Op(Op::Ne)),
            ("!=", Token::Op(Op::Ne)),
            ("==", Token::Op(Op::Eq)),
            ("=", Token::Op(Op::Eq)),
            ("<", Token::Op(Op::Lt)),
            (">", Token::Op(Op::Gt)),
        ];
        if let Some((symbol, token)) = symbols.iter().find(|(symbol, _)| rest.starts_with(symbol)) {
            tokens.push(token.clone());
            rest = &rest[symbol.len()..];
            continue;
        }

        // `~r/^[\w.-]+/`, where `\w` is ASCII
        let length = rest
            .bytes()
            .take_while(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
            .count();
        if length == 0 {
            return Err(format!("unexpected character at {rest:?}"));
        }
        tokens.push(word_token(&rest[..length]));
        rest = &rest[length..];
    }

    Ok(tokens)
}

fn word_token(word: &str) -> Token {
    match word.to_lowercase().as_str() {
        "and" => Token::And,
        "or" => Token::Or,
        "true" => Token::Literal(Value::Bool(true)),
        "false" => Token::Literal(Value::Bool(false)),
        "empty" => Token::Op(Op::Empty),
        "notempty" => Token::Op(Op::NotEmpty),
        "contains" | "contain" => Token::Op(Op::Contains),
        "notcontains" | "notcontain" => Token::Op(Op::NotContains),
        "anyof" => Token::Op(Op::AnyOf),
        "allof" => Token::Op(Op::AllOf),
        "noneof" => Token::Op(Op::NoneOf),
        _ => match elixir::parse_number(word) {
            Some(number) => Token::Literal(number),
            None => Token::Error(format!("unrecognized token {word:?}")),
        },
    }
}

fn parse(tokens: &[Token]) -> Result<Ast, String> {
    if let Some(Token::Error(reason)) = tokens.iter().find(|t| matches!(t, Token::Error(_))) {
        return Err(reason.clone());
    }
    match parse_or(tokens)? {
        (ast, []) => Ok(ast),
        (_, rest) => Err(format!("unexpected trailing tokens {rest:?}")),
    }
}

type Parsed<'a> = Result<(Ast, &'a [Token]), String>;

fn parse_or(tokens: &[Token]) -> Parsed<'_> {
    let (mut left, mut rest) = parse_and(tokens)?;
    while let [Token::Or, after @ ..] = rest {
        let (right, after) = parse_and(after)?;
        left = Ast::Or(Box::new(left), Box::new(right));
        rest = after;
    }
    Ok((left, rest))
}

fn parse_and(tokens: &[Token]) -> Parsed<'_> {
    let (mut left, mut rest) = parse_primary(tokens)?;
    while let [Token::And, after @ ..] = rest {
        let (right, after) = parse_primary(after)?;
        left = Ast::And(Box::new(left), Box::new(right));
        rest = after;
    }
    Ok((left, rest))
}

fn parse_primary(tokens: &[Token]) -> Parsed<'_> {
    if let [Token::LParen, rest @ ..] = tokens {
        let (inner, rest) = parse_or(rest)?;
        return match rest {
            [Token::RParen, rest @ ..] => Ok((inner, rest)),
            _ => Err("expected closing parenthesis".into()),
        };
    }
    let (left, rest) = parse_operand(tokens)?;
    match rest {
        [Token::Op(op @ (Op::Empty | Op::NotEmpty)), rest @ ..] => {
            Ok((Ast::Unary(*op, left), rest))
        }
        [Token::Op(op), rest @ ..] => {
            let (right, rest) = parse_operand(rest)?;
            Ok((Ast::Binary(*op, left, right), rest))
        }
        // A bare operand, like `{field}`, is true when it has a value
        _ => Ok((Ast::Unary(Op::NotEmpty, left), rest)),
    }
}

fn parse_operand(tokens: &[Token]) -> Result<(Operand, &[Token]), String> {
    match tokens {
        [Token::Field(name), rest @ ..] => Ok((Operand::Field(name.clone()), rest)),
        [Token::Literal(value), rest @ ..] => Ok((Operand::Literal(value.clone()), rest)),
        [Token::LBracket, rest @ ..] => parse_array(rest),
        _ => Err("expected operand".into()),
    }
}

fn parse_array(tokens: &[Token]) -> Result<(Operand, &[Token]), String> {
    let mut items = Vec::new();
    let mut rest = tokens;
    if let [Token::RBracket, after @ ..] = rest {
        return Ok((Operand::Literal(Value::Array(items)), after));
    }
    loop {
        match rest {
            [Token::Literal(value), after @ ..] => {
                items.push(value.clone());
                rest = after;
            }
            _ => return Err("expected array element".into()),
        }
        match rest {
            [Token::Comma, after @ ..] => rest = after,
            [Token::RBracket, after @ ..] => {
                return Ok((Operand::Literal(Value::Array(items)), after));
            }
            _ => return Err("expected comma or closing bracket in array".into()),
        }
    }
}

fn eval(ast: &Ast, scope: Scope) -> bool {
    match ast {
        Ast::Or(left, right) => eval(left, scope) || eval(right, scope),
        Ast::And(left, right) => eval(left, scope) && eval(right, scope),
        Ast::Unary(Op::Empty, operand) => !has_value(&resolve(operand, scope)),
        Ast::Unary(_, operand) => has_value(&resolve(operand, scope)),
        Ast::Binary(op, left, right) => {
            let (left, right) = (resolve(left, scope), resolve(right, scope));
            match op {
                Op::Eq => loose_equals(&left, &right),
                Op::Ne => !loose_equals(&left, &right),
                Op::Gt | Op::Lt | Op::Ge | Op::Le => compare(*op, &left, &right),
                Op::Contains => contains(&left, &right),
                Op::NotContains => !contains(&left, &right),
                Op::AnyOf => any_of(&left, &right),
                Op::AllOf => {
                    let left = wrap(&left);
                    wrap(&right)
                        .iter()
                        .all(|item| left.iter().any(|l| loose_equals(item, l)))
                }
                Op::NoneOf => !any_of(&left, &right),
                // `empty` and `notempty` take no right operand: the parser
                // reads them as unary
                Op::Empty | Op::NotEmpty => false,
            }
        }
    }
}

fn any_of(left: &Value, right: &Value) -> bool {
    let right = wrap(right);
    wrap(left)
        .iter()
        .any(|item| right.iter().any(|r| loose_equals(item, r)))
}

fn resolve(operand: &Operand, scope: Scope) -> Value {
    match operand {
        Operand::Field(name) => scope.get(name),
        Operand::Literal(value) => value.clone(),
    }
}

/// `List.wrap/1`.
fn wrap(value: &Value) -> Vec<Value> {
    match value {
        Value::Null => Vec::new(),
        Value::Array(items) => items.clone(),
        other => vec![other.clone()],
    }
}

/// `Visibility.loose_equals?/2`: form params arrive as strings, so `"5"`
/// equals `5`, `"true"` equals `true`, and `nil` equals `false`.
pub fn loose_equals(left: &Value, right: &Value) -> bool {
    match (left, right) {
        _ if left == right => true,
        (Value::Null, Value::Bool(false)) | (Value::Bool(false), Value::Null) => true,
        (Value::String(text), Value::Bool(true)) | (Value::Bool(true), Value::String(text)) => {
            text == "true"
        }
        (Value::String(text), Value::Bool(false)) | (Value::Bool(false), Value::String(text)) => {
            text == "false"
        }
        _ if left.is_number() || right.is_number() => match (to_number(left), to_number(right)) {
            (Some(left), Some(right)) => left == right,
            _ => false,
        },
        _ => false,
    }
}

fn compare(op: Op, left: &Value, right: &Value) -> bool {
    if left.is_null() || right.is_null() {
        return false;
    }
    let ordering = match (to_number(left), to_number(right), left, right) {
        (Some(left), Some(right), _, _) => left.partial_cmp(&right),
        (_, _, Value::String(left), Value::String(right)) => {
            Some(left.as_bytes().cmp(right.as_bytes()))
        }
        _ => None,
    };
    let Some(ordering) = ordering else {
        return false;
    };
    match op {
        Op::Gt => ordering.is_gt(),
        Op::Lt => ordering.is_lt(),
        Op::Ge => ordering.is_ge(),
        _ => ordering.is_le(),
    }
}

fn contains(value: &Value, item: &Value) -> bool {
    match (value, item) {
        (Value::Array(items), _) => items.iter().any(|candidate| loose_equals(candidate, item)),
        (Value::String(text), Value::String(part)) => text.contains(part.as_str()),
        _ => false,
    }
}

fn to_number(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => elixir::parse_number(text).and_then(|number| number.as_f64()),
        _ => None,
    }
}

/// `Visibility.has_value?/1`.
fn has_value(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::String(text) => !text.is_empty(),
        Value::Array(items) => !items.is_empty(),
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn met(expression: &str, values: Value, keys: Keys) -> bool {
        let values = values.as_object().cloned().unwrap();
        evaluate(expression, Scope::new(&values, keys))
    }

    #[test]
    fn compares_loosely_as_form_params_are_strings() {
        let params = json!({"n": "5", "flag": "true", "name": "Ada", "list": ["a", "b"]});
        let met = |expression| met(expression, params.clone(), Keys::Strings);
        assert!(met("{n} = 5"));
        assert!(met("{n} > 4.5 and {n} <= 5"));
        assert!(met("{flag} = true"));
        assert!(met("{missing} = false"));
        assert!(met("{name} contains 'd'"));
        assert!(met("{list} anyof ['b', 'c']"));
        assert!(!met("{list} allof ['b', 'c']"));
        assert!(met("{list} noneof ['c']"));
        assert!(!met("({missing} notempty or {name}) and not_a_word = 1"));
        assert!(met("{name} <> 'Bob'"));
        assert!(met("{missing} empty"));
        assert!(!met("{name"));
    }

    #[test]
    fn false_reads_as_nil_over_string_keys_only() {
        let values = json!({"agree": false});
        assert!(!met("{agree} notempty", values.clone(), Keys::Strings));
        assert!(met("{agree} notempty", values, Keys::Atoms));
    }
}
