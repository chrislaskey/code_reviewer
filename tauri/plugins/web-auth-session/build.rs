// The Swift package in `ios/`, compiled and linked on iOS. No command is
// called from JavaScript: the app's Rust calls the plugin.
const COMMANDS: &[&str] = &[];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).ios_path("ios").build();
}
