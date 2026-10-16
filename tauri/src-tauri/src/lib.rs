use code_reviewer_core::{
    self as core, GreetRequest,
    git::{ChangedFile, ListChangedFilesRequest},
};

#[tauri::command]
fn greet(name: &str) -> String {
    let response = core::greet(GreetRequest {
        name: name.to_string(),
    });
    response.message
}

#[tauri::command]
fn list_changed_files(repo_path: &str) -> Result<Vec<ChangedFile>, String> {
    core::git::list_changed_files(ListChangedFilesRequest {
        repo_path: repo_path.to_string(),
    })
    .map(|r| r.files)
    .map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet, list_changed_files])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
