fn main() {
    // The server URL compiled into a mobile build, see src/sync.rs
    println!("cargo:rerun-if-env-changed=SYNC_SERVER_URL");
    tauri_build::build()
}
