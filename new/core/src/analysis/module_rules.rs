use serde::{Deserialize, Serialize};

use super::model::FunctionId;

/// Which calls a review shows, beyond what the project itself defines.
///
/// Each pattern names a module, a function, or a prefix:
/// - `Shop` matches `Shop` and `Shop.Orders`, but not `ShopWeb`.
/// - `Shop.Orders.place` matches that function at any arity.
/// - `Shop.Orders.place/2` matches one arity.
/// - `Shop*` matches anything that starts with `Shop`, so `ShopWeb` and `ShopTest` too.
///
/// Ignoring wins over including.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleRules {
    /// Treat matching calls like project calls, even though the project does not define them.
    #[serde(default)]
    pub include: Vec<String>,
    /// Leave matching callers and calls out of the review.
    #[serde(default)]
    pub ignore: Vec<String>,
}

impl ModuleRules {
    /// Should this function be left out of the review?
    pub fn is_ignored(&self, function: &FunctionId) -> bool {
        matches_any(&self.ignore, function)
    }

    /// Does the rule list make this function count as a project function?
    pub fn is_included(&self, function: &FunctionId) -> bool {
        matches_any(&self.include, function)
    }
}

fn matches_any(patterns: &[String], function: &FunctionId) -> bool {
    patterns
        .iter()
        .map(|pattern| pattern.trim())
        .filter(|pattern| !pattern.is_empty())
        .any(|pattern| matches_pattern(pattern, function))
}

fn matches_pattern(pattern: &str, function: &FunctionId) -> bool {
    let module_and_name = format!("{}.{}", function.module, function.name);
    let full_name = format!("{module_and_name}/{}", function.arity);

    if let Some(prefix) = pattern.strip_suffix('*') {
        return full_name.starts_with(prefix);
    }
    function.module == pattern
        || function.module.starts_with(&format!("{pattern}."))
        || module_and_name == pattern
        || full_name == pattern
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(include: &[&str], ignore: &[&str]) -> ModuleRules {
        ModuleRules {
            include: include.iter().map(|p| p.to_string()).collect(),
            ignore: ignore.iter().map(|p| p.to_string()).collect(),
        }
    }

    fn function(module: &str, name: &str, arity: usize) -> FunctionId {
        FunctionId::new(module, name, arity)
    }

    #[test]
    fn test_module_pattern_matches_the_module_and_its_children_only() {
        let rules = rules(&["Shop"], &[]);

        assert!(rules.is_included(&function("Shop", "run", 0)));
        assert!(rules.is_included(&function("Shop.Orders", "place", 1)));
        assert!(!rules.is_included(&function("ShopWeb.Router", "init", 1)));
        assert!(!rules.is_included(&function("ShopTest", "setup", 0)));
    }

    #[test]
    fn test_star_matches_a_raw_prefix() {
        let rules = rules(&["Shop*"], &[]);

        assert!(rules.is_included(&function("ShopWeb.Router", "init", 1)));
        assert!(rules.is_included(&function("ShopTest", "setup", 0)));
        assert!(!rules.is_included(&function("Other", "run", 0)));
    }

    #[test]
    fn test_function_patterns_match_with_or_without_arity() {
        let rules = rules(&[], &["Enum.reduce", "Map.get/3"]);

        assert!(rules.is_ignored(&function("Enum", "reduce", 3)));
        assert!(rules.is_ignored(&function("Enum", "reduce", 2)));
        assert!(!rules.is_ignored(&function("Enum", "map", 2)));
        assert!(rules.is_ignored(&function("Map", "get", 3)));
        assert!(!rules.is_ignored(&function("Map", "get", 2)));
    }

    #[test]
    fn test_blank_patterns_match_nothing() {
        let rules = rules(&["", "  "], &[""]);

        assert!(!rules.is_included(&function("Shop", "run", 0)));
        assert!(!rules.is_ignored(&function("Shop", "run", 0)));
    }
}
