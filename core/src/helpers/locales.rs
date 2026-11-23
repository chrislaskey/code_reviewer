//! The languages the pages are translated into: English, the language
//! they are written in, and one for each `svelte/translations/<locale>.po`
//! and `svelte/translations/public/<locale>.po`, the public website's,
//! which a test keeps in step. A person's choice is one of them, see
//! `accounts::preferences` and `docs/design/locales.md`.

/// The language the pages are written in, shown when no other applies.
pub const DEFAULT: &str = "en";

/// Every language a person may choose, the default first.
pub const SUPPORTED: &[&str] = &["en", "es"];

/// Whether the language is one a person may choose.
pub fn is_supported(locale: &str) -> bool {
    SUPPORTED.contains(&locale)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The languages a folder of translations has a `.po` for.
    fn translated_in(folder: &str) -> Vec<String> {
        let folder = format!(
            "{}/../svelte/translations/{folder}",
            env!("CARGO_MANIFEST_DIR")
        );
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
            translated_in(""),
            "SUPPORTED and svelte/translations/*.po"
        );
        assert_eq!(
            supported,
            translated_in("public"),
            "SUPPORTED and svelte/translations/public/*.po"
        );
        assert_eq!(SUPPORTED[0], DEFAULT);
    }

    #[test]
    fn locales_is_supported_only_for_those_listed() {
        assert!(is_supported("en"));
        assert!(!is_supported("xx"));
        assert!(!is_supported(""));
    }
}
