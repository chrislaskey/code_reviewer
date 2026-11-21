//! The repository's own framework files against its `framework.lock`:
//! part of `cargo test --workspace`, so an undeclared change to the
//! template's code fails the build like a lint.

use std::path::Path;

#[test]
fn framework_check_finds_the_framework_files_as_locked() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the repository's root");
    let problems = framework_check::check(root).expect("the check runs");
    assert!(
        problems.is_empty(),
        "{}",
        framework_check::explained(&problems)
    );
}
