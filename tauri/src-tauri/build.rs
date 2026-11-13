fn main() {
    // Listing the app's commands generates an `allow-<command>` permission for
    // each, which `capabilities/default.json` references.
    let manifest = tauri_build::AppManifest::new().commands(&[
        "list_branches_and_recent_commits",
        "analyze_file_changes",
        "get_function_diffs",
        "get_file_diffs",
        "list_tests",
        "get_test_call_stack",
        "get_session_settings",
        "get_default_session_port",
        "list_sessions",
        "get_session",
        "get_agent_chat_history",
        "send_prompt_to_session",
        "create_session",
        "continue_session",
        "merge_session",
        "close_session",
        "show_session_in_terminal_app",
        "open_session_in_browser",
        "get_app_platform",
        "get_remote_access_status",
        "set_remote_access_enabled",
        "create_paired_device",
        "revoke_paired_device",
        "get_pairing_status",
        "pair_with_computer",
        "unpair_from_computer",
    ]);

    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest))
        .expect("failed to run tauri-build");
}
