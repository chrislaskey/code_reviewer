//! What was wrong with a form's fields, by field: the messages a page
//! shows under each input, as Ecto's `changeset.errors` are. Rules are
//! declared on the request's struct with `garde`, and its report becomes
//! this map. The web server answers it as `{"errors": {...}}` with 422,
//! and the app's commands answer the same shape.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

/// The messages for each field that was wrong, by the field's name, as
/// the request names it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldErrors(pub BTreeMap<String, Vec<String>>);

impl FieldErrors {
    /// Checks the value against the rules declared on its struct.
    pub fn check<T: garde::Validate<Context = ()>>(value: &T) -> Result<(), FieldErrors> {
        value.validate().map_err(FieldErrors::from)
    }

    /// One field's message, for a rule no struct declares.
    pub fn one(field: &str, message: impl Into<String>) -> FieldErrors {
        FieldErrors(BTreeMap::from([(field.to_string(), vec![message.into()])]))
    }
}

impl From<garde::Report> for FieldErrors {
    fn from(report: garde::Report) -> FieldErrors {
        let mut errors: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (path, error) in report.iter() {
            errors
                .entry(path.to_string())
                .or_default()
                .push(error.message().to_string());
        }
        FieldErrors(errors)
    }
}

/// Every message, as `field: message`, for a log or a plain error string.
impl fmt::Display for FieldErrors {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        for (field, messages) in &self.0 {
            for message in messages {
                if !first {
                    formatter.write_str("; ")?;
                }
                write!(formatter, "{field}: {message}")?;
                first = false;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use garde::Validate;

    #[derive(Validate)]
    struct Signup {
        #[garde(length(min = 1))]
        name: String,
        #[garde(custom(not_admin))]
        handle: String,
    }

    fn not_admin(value: &str, _context: &()) -> garde::Result {
        if value == "admin" {
            return Err(garde::Error::new("is taken"));
        }
        Ok(())
    }

    #[test]
    fn field_errors_check_collects_the_messages_by_field() {
        let wrong = Signup {
            name: String::new(),
            handle: "admin".to_string(),
        };
        let errors = FieldErrors::check(&wrong).expect_err("two fields wrong");
        assert_eq!(errors.0.keys().collect::<Vec<_>>(), ["handle", "name"]);
        assert_eq!(errors.0["handle"], ["is taken"]);
        assert!(errors.to_string().starts_with("handle: is taken; name: "));

        let right = Signup {
            name: "Alice".to_string(),
            handle: "alice".to_string(),
        };
        assert_eq!(FieldErrors::check(&right), Ok(()));
    }
}
