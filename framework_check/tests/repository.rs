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

#[test]
fn framework_check_leaves_an_apps_new_files_beside_the_templates_to_the_app() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the repository's root");
    let manifest = framework_check::manifest(root).expect("the manifest");
    for path in [
        "axum/src/app/recipes.rs",
        "axum/src/tests/recipes.rs",
        "tauri/src/app/recipes.rs",
        "core/src/app/recipes.rs",
        "core/src/services/tmux.rs",
        "core/src/helpers/durations.rs",
        "svelte/src/lib/recipe_card.js",
        "bin/artifacts",
    ] {
        assert_eq!(
            framework_check::kind_of(&manifest, path).expect("a kind"),
            framework_check::Kind::App,
            "{path}"
        );
    }
    for path in [
        "axum/src/main.rs",
        "axum/src/app/communities.rs",
        "tauri/src/sync.rs",
        "core/src/services/web_server.rs",
        "svelte/src/lib/components/Button.svelte",
        "bin/lint",
    ] {
        assert_eq!(
            framework_check::kind_of(&manifest, path).expect("a kind"),
            framework_check::Kind::Framework,
            "{path}"
        );
    }
}

#[test]
fn framework_check_finds_no_example_named_in_the_framework() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the repository's root");
    let manifest = framework_check::manifest(root).expect("the manifest");
    let named = framework_check::examples::list_named_in_framework(root, &manifest)
        .expect("the framework's files");
    assert!(
        named.is_empty(),
        "The framework names an example, which an app deleting it would break, \
         see docs/design/examples-delete.md. Give the example's part to it, \
         through a SyncedTable method say:\n\n{}",
        named
            .iter()
            .map(|named| format!("  {named}\n"))
            .collect::<String>()
    );
}
