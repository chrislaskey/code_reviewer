//! The languages the pages are translated into: English, the language
//! they are written in, and one for each `svelte/translations/<locale>.po`
//! and `svelte/translations/public/<locale>.po`, the public website's,
//! and `core/translations/<locale>.po`, the emails', which a test keeps in
//! step. A person's choice is one of them, see
//! `accounts::preferences` and `docs/design/locales.md`.

/// The language the pages are written in, shown when no other applies.
pub const DEFAULT: &str = "en";

/// Every language a person may choose, the default first.
pub const SUPPORTED: &[&str] = &["en", "es"];

/// Whether the language is one a person may choose.
pub fn is_supported(locale: &str) -> bool {
    SUPPORTED.contains(&locale)
}

/// The first language of an `Accept-Language` header the pages are in,
/// `es-MX` as `es`, in the order the header lists them; English when
/// none is.
pub fn resolve(accept_language: &str) -> &'static str {
    for tag in accept_language.split(',') {
        let tag = tag
            .split(';')
            .next()
            .unwrap_or_default()
            .trim()
            .to_lowercase();
        let language = tag.split('-').next().unwrap_or_default();
        if let Some(locale) = SUPPORTED
            .iter()
            .find(|locale| **locale == tag || **locale == language)
        {
            return locale;
        }
    }
    DEFAULT
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The languages a folder of translations has a `.po` for.
    fn translated_in(folder: &str) -> Vec<String> {
        let folder = format!("{}/../{folder}", env!("CARGO_MANIFEST_DIR"));
        let mut translated: Vec<String> = std::fs::read_dir(&folder)
            .expect("the translations folder")
            .map(|entry| {
                entry
                    .expect("an entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .filter_map(|name| name.strip_suffix(".po").map(str::to_string))
            .collect();
        translated.sort();
        translated
    }

    #[test]
    fn locales_supported_are_the_default_and_the_translations() {
        let mut supported: Vec<String> = SUPPORTED
            .iter()
            .filter(|locale| **locale != DEFAULT)
            .map(|locale| locale.to_string())
            .collect();
        supported.sort();
        assert_eq!(
            supported,
            translated_in("svelte/translations"),
            "SUPPORTED and svelte/translations/*.po"
        );
        assert_eq!(
            supported,
            translated_in("svelte/translations/public"),
            "SUPPORTED and svelte/translations/public/*.po"
        );
        assert_eq!(
            supported,
            translated_in("core/translations"),
            "SUPPORTED and core/translations/*.po"
        );
        assert_eq!(SUPPORTED[0], DEFAULT);
    }

    #[test]
    fn locales_resolve_takes_the_first_language_listed_the_pages_are_in() {
        assert_eq!(resolve("es-MX,es;q=0.9,en;q=0.8"), "es");
        assert_eq!(resolve("fr-FR, en-GB;q=0.8"), "en");
        assert_eq!(resolve("ES"), "es");
        assert_eq!(resolve("fr"), DEFAULT);
        assert_eq!(resolve(""), DEFAULT);
    }

    #[test]
    fn locales_is_supported_only_for_those_listed() {
        assert!(is_supported("en"));
        assert!(!is_supported("xx"));
        assert!(!is_supported(""));
    }
}
