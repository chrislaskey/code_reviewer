use globset::{GlobBuilder, GlobMatcher};
use regex::Regex;

/// Patterns starting with this prefix are regular expressions, not globs.
pub const REGEX_PREFIX: &str = "regex:";

/// One compiled include or exclude pattern, matched against a repo-relative path.
///
/// - Glob (default): matches the whole path. `*` stays inside one directory,
///   `**` crosses directories, so `**/*_test.ex` matches a test file anywhere.
/// - `regex:<expression>`: matches anywhere in the path. Anchor with `^` and `$`.
#[derive(Debug)]
pub(super) enum PathPattern {
    Glob(GlobMatcher),
    Regex(Regex),
}

impl PathPattern {
    /// Compile a pattern. The error is a short message for the caller to wrap.
    pub(super) fn compile(pattern: &str) -> Result<Self, String> {
        let pattern = pattern.trim();
        if pattern.is_empty() {
            return Err("pattern is empty".to_string());
        }
        if let Some(expression) = pattern.strip_prefix(REGEX_PREFIX) {
            return Regex::new(expression)
                .map(PathPattern::Regex)
                .map_err(|error| error.to_string());
        }
        GlobBuilder::new(pattern)
            .literal_separator(true)
            .build()
            .map(|glob| PathPattern::Glob(glob.compile_matcher()))
            .map_err(|error| error.to_string())
    }

    pub(super) fn is_match(&self, path: &str) -> bool {
        match self {
            PathPattern::Glob(matcher) => matcher.is_match(path),
            PathPattern::Regex(regex) => regex.is_match(path),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_match(pattern: &str, path: &str) -> bool {
        PathPattern::compile(pattern).unwrap().is_match(path)
    }

    #[test]
    fn test_double_star_crosses_directories_and_single_star_does_not() {
        assert!(is_match("lib/**/*.ex", "lib/shop/orders.ex"));
        assert!(!is_match("lib/*.ex", "lib/shop/orders.ex"));
        assert!(is_match("lib/*.ex", "lib/shop.ex"));
    }

    #[test]
    fn test_leading_double_star_also_matches_the_top_level() {
        assert!(is_match("**/*_test.exs", "shop_test.exs"));
        assert!(is_match("**/*_test.exs", "test/shop/orders_test.exs"));
    }

    #[test]
    fn test_glob_without_directories_only_matches_the_top_level() {
        assert!(is_match("*.md", "README.md"));
        assert!(!is_match("*.md", "docs/guide.md"));
    }

    #[test]
    fn test_braces_choose_between_alternatives() {
        assert!(is_match("**/*.{ex,exs}", "lib/shop.ex"));
        assert!(is_match("**/*.{ex,exs}", "test/shop_test.exs"));
        assert!(!is_match("**/*.{ex,exs}", "lib/shop.rs"));
    }

    #[test]
    fn test_regex_prefix_matches_anywhere_in_the_path() {
        assert!(is_match("regex:_(test|spec)\\.", "test/shop_test.exs"));
        assert!(!is_match("regex:^test/", "lib/test/shop.ex"));
    }

    #[test]
    fn test_blank_and_broken_patterns_are_rejected() {
        assert!(PathPattern::compile("  ").is_err());
        assert!(PathPattern::compile("regex:(").is_err());
        assert!(PathPattern::compile("lib/[").is_err());
    }
}
