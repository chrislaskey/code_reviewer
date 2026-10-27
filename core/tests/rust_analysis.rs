//! Rust files across a small workspace, indexed in memory: calls resolve
//! through `use` paths, `self`, glob imports in test modules, method names
//! and another crate's package name.

use code_reviewer_core::analysis::external_calls::list_external_calls_for_function;
use code_reviewer_core::analysis::function_changes::FunctionChangeKind;
use code_reviewer_core::analysis::model::FunctionId;
use code_reviewer_core::analysis::project_index::{
    build_project_index_from_sources, CallDirection, ProjectIndex, SourceFile,
};
use code_reviewer_core::analysis::test_call_stack::list_tests;

const LIB: &str = r#"pub mod billing;
pub mod orders;
"#;

const ORDERS: &str = r#"use crate::billing::Billing;

pub struct Orders;

impl Orders {
    pub fn place(&self, order: u32) -> u32 {
        let charged = Billing::charge(order, 1);
        self.validate(charged)
    }

    fn validate(&self, order: u32) -> u32 {
        std::cmp::max(order, 0)
    }
}

pub fn summarize(orders: &Orders) -> u32 {
    orders.place(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_place() {
        assert_eq!(Orders.place(1), 1);
        summarize(&Orders);
    }
}
"#;

const BILLING: &str = r#"pub struct Billing;

impl Billing {
    pub fn charge(order: u32, method: u32) -> u32 {
        order + method
    }
}
"#;

const APP_MAIN: &str = r#"use shop_core::orders::Orders;

fn main() {
    let orders = Orders;
    orders.place(2);
    shop_core::orders::summarize(&orders);
}
"#;

fn sample_index() -> ProjectIndex {
    let source = |path: &str, text: &str| SourceFile {
        path: path.to_string(),
        text: text.to_string(),
    };
    build_project_index_from_sources(vec![
        source("core/src/lib.rs", LIB),
        source("core/src/orders.rs", ORDERS),
        source("core/src/billing.rs", BILLING),
        source("app/src/main.rs", APP_MAIN),
        source("README.md", "not code"),
    ])
}

fn id(module: &str, name: &str, arity: usize) -> FunctionId {
    FunctionId::new(module, name, arity)
}

#[test]
fn test_functions_are_indexed_by_crate_module_and_type() {
    let index = sample_index();
    let ids: Vec<String> = index.list_functions().map(|f| f.id.to_string()).collect();
    assert_eq!(
        ids,
        vec![
            "app.main/0",
            "core::billing::Billing.charge/2",
            "core::orders.summarize/1",
            "core::orders::Orders.place/1",
            "core::orders::Orders.validate/1",
            "core::orders::tests.test_place/0",
        ]
    );
    assert_eq!(index.skipped_files().len(), 1);
}

#[test]
fn test_calls_resolve_through_use_paths_self_and_method_names() {
    let index = sample_index();
    let calls = list_external_calls_for_function(&index, &id("core::orders::Orders", "place", 1))
        .expect("function exists");

    let out: Vec<(String, bool)> = calls
        .calls_out
        .iter()
        .map(|link| (link.function.to_string(), link.defined_in_project))
        .collect();
    assert_eq!(
        out,
        vec![("core::billing::Billing.charge/2".to_string(), true)]
    );

    // `self.validate` stays inside the type, so it is not an external call.
    let within = index.list_call_links(
        &id("core::orders::Orders", "place", 1),
        CallDirection::OutgoingCalls,
    );
    assert!(within.iter().any(
        |link| link.function == id("core::orders::Orders", "validate", 1)
            && !link.crosses_module_boundary
    ));

    let callers: Vec<String> = calls
        .calls_in
        .iter()
        .map(|link| link.function.to_string())
        .collect();
    assert_eq!(
        callers,
        vec![
            "app.main/0",
            "core::orders.summarize/1",
            "core::orders::tests.test_place/0",
        ]
    );
}

#[test]
fn test_calls_from_another_crate_match_by_the_path_after_the_package_name() {
    let index = sample_index();
    let links = index.list_call_links(&id("app", "main", 0), CallDirection::OutgoingCalls);
    let targets: Vec<(String, bool)> = links
        .iter()
        .map(|link| (link.function.to_string(), link.defined_in_project))
        .collect();
    assert_eq!(
        targets,
        vec![
            ("core::orders.summarize/1".to_string(), true),
            ("core::orders::Orders.place/1".to_string(), true),
        ]
    );
}

#[test]
fn test_std_calls_are_kept_as_calls_outside_the_project() {
    let index = sample_index();
    let links = index.list_call_links(
        &id("core::orders::Orders", "validate", 1),
        CallDirection::OutgoingCalls,
    );
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].function, id("std::cmp", "max", 2));
    assert!(!links[0].defined_in_project);
}

#[test]
fn test_rust_test_functions_are_listed_as_tests() {
    let index = sample_index();
    let changed = std::collections::HashMap::from([(
        id("core::orders::tests", "test_place", 0),
        FunctionChangeKind::Modified,
    )]);
    let tests = list_tests(&index, &changed);
    assert_eq!(tests.len(), 1);
    assert_eq!(
        tests[0].function,
        id("core::orders::tests", "test_place", 0)
    );
    assert_eq!(tests[0].file_path, "core/src/orders.rs");
    assert_eq!(tests[0].change, Some(FunctionChangeKind::Modified));
}
