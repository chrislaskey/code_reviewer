fn main() {
    // Listing the app's commands generates an `allow-<command>` permission for
    // each, which `capabilities/default.json` references.
    let manifest = tauri_build::AppManifest::new().commands(&[
        "list_branches_and_recent_commits",
        "analyze_file_changes",
        "get_function_diffs",
        "get_file_diffs",
        "get_session_settings",
        "list_sessions",
        "create_session",
        "show_session_in_terminal_app",
    ]);

    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest))
        .expect("failed to run tauri-build");
}
