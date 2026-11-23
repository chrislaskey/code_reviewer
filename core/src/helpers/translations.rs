//! The text Rust itself shows a person, the emails, in their language.
//! The English is written in `t!`, with named placeholders,
//! `t!(locale, "Your code is {code}.", code = code)`; its translations are
//! the catalog `core/translations/`, apart from the pages' two, extracted
//! from every `t!` and compiled to `<locale>.mo` by `pnpm
//! translations:update` (svelte/). English shows where a language has no
//! catalog or no translation. See `docs/design/locales.md`.

use std::collections::HashMap;
use std::sync::OnceLock;

use gettext::Catalog;

use crate::app;

fn catalogs() -> &'static HashMap<&'static str, Catalog> {
    static PARSED: OnceLock<HashMap<&'static str, Catalog>> = OnceLock::new();
    PARSED.get_or_init(|| {
        app::locales::CATALOGS
            .iter()
            .filter_map(|(locale, bytes)| Some((*locale, Catalog::parse(*bytes).ok()?)))
            .collect()
    })
}

/// The English in the language, or as it is.
pub fn translated(locale: &str, english: &str) -> String {
    match catalogs().get(locale) {
        Some(catalog) => catalog.gettext(english).to_string(),
        None => english.to_string(),
    }
}

/// The text with each `{name}` placeholder given its value.
pub fn filled(text: String, values: &[(&str, &str)]) -> String {
    values.iter().fold(text, |text, (name, value)| {
        text.replace(&format!("{{{name}}}"), value)
    })
}

/// English translated into the language, its placeholders filled:
/// `t!(locale, "Your code is {code}.", code = code)`. The English must be
/// a literal, for the extraction to read it.
macro_rules! t {
    ($locale:expr, $english:literal $(, $name:ident = $value:expr)* $(,)?) => {
        $crate::helpers::translations::filled(
            $crate::helpers::translations::translated($locale, $english),
            &[$((stringify!($name), &$value.to_string())),*],
        )
    };
}
pub(crate) use t;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::helpers::locales;

    #[test]
    fn translations_have_a_catalog_for_each_language_but_english_that_parses() {
        let mut compiled: Vec<&str> = app::locales::CATALOGS
            .iter()
            .map(|(locale, _)| *locale)
            .collect();
        compiled.sort();
        let mut supported: Vec<&str> = app::locales::SUPPORTED
            .iter()
            .copied()
            .filter(|locale| *locale != locales::DEFAULT)
            .collect();
        supported.sort();
        assert_eq!(compiled, supported, "app::locales: CATALOGS and SUPPORTED");
        assert_eq!(
            catalogs().len(),
            app::locales::CATALOGS.len(),
            "every catalog parses"
        );
    }

    #[test]
    fn translations_fill_placeholders_and_keep_the_english_without_a_catalog() {
        let code = "123456";
        assert_eq!(
            filled(translated("en", "Your code is {code}."), &[("code", code)]),
            "Your code is 123456."
        );
        assert_eq!(translated("xx", "Not translated"), "Not translated");
    }
}
