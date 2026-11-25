//! The daisyUI themes a person may choose, the pages' list in
//! `svelte/src/lib/layouts/themes.js`, which a test keeps in step. A
//! person's choice is one of them, see `accounts::preferences`.

/// Every theme the pages are built with, `themes: all` in
/// `svelte/src/lib/theme.css`.
pub const SUPPORTED: &[&str] = &[
    "light",
    "dark",
    "abyss",
    "acid",
    "aqua",
    "autumn",
    "black",
    "bumblebee",
    "business",
    "caramellatte",
    "cmyk",
    "coffee",
    "corporate",
    "cupcake",
    "cyberpunk",
    "dim",
    "dracula",
    "emerald",
    "fantasy",
    "forest",
    "garden",
    "halloween",
    "lemonade",
    "lofi",
    "luxury",
    "night",
    "nord",
    "pastel",
    "retro",
    "silk",
    "sunset",
    "synthwave",
    "valentine",
    "winter",
    "wireframe",
];

/// Whether the theme is one a person may choose.
pub fn is_supported(theme: &str) -> bool {
    SUPPORTED.contains(&theme)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The quoted names of the `THEMES` list in the pages' themes.js.
    fn listed_in_pages() -> Vec<String> {
        let path = format!(
            "{}/../svelte/src/lib/layouts/themes.js",
            env!("CARGO_MANIFEST_DIR")
        );
        let source = std::fs::read_to_string(&path).expect("themes.js");
        let list = source
            .split_once("THEMES = [")
            .and_then(|(_, rest)| rest.split_once(']'))
            .map(|(list, _)| list)
            .expect("the THEMES list");
        list.split(',')
            .map(|name| name.trim().trim_matches('"').to_string())
            .filter(|name| !name.is_empty())
            .collect()
    }

    #[test]
    fn themes_supported_are_the_pages_list() {
        assert_eq!(SUPPORTED, listed_in_pages(), "SUPPORTED and themes.js");
    }

    #[test]
    fn themes_is_supported_only_for_those_listed() {
        assert!(is_supported("dracula"));
        assert!(!is_supported("system"));
        assert!(!is_supported(""));
    }
}
