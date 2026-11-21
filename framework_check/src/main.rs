//! `cargo run -p framework_check -- check|lock|hook`: checks the
//! template's own files against `framework.lock`, rewrites the lock, or
//! answers a Claude Code `PreToolUse` hook about one edit, from the
//! repository's root, see the library.

use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or(Path::new("."));
    let command = std::env::args().nth(1);
    match command.as_deref() {
        Some("check") | None => check(root),
        Some("lock") => match framework_check::write_lock(root) {
            Ok(count) => {
                println!("{} written: {count} framework files", framework_check::LOCK);
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("{error}");
                ExitCode::FAILURE
            }
        },
        Some("hook") => hook(root),
        Some(other) => {
            eprintln!("unknown command {other}; commands: check, lock, hook");
            ExitCode::FAILURE
        }
    }
}

/// Reads the hook's JSON on stdin and prints its answer, if any. A hook
/// that fails must never stop an edit, so every failure answers nothing.
fn hook(root: &Path) -> ExitCode {
    let mut input = String::new();
    if std::io::Read::read_to_string(&mut std::io::stdin(), &mut input).is_err() {
        return ExitCode::SUCCESS;
    }
    let Ok(event) = serde_json::from_str::<serde_json::Value>(&input) else {
        return ExitCode::SUCCESS;
    };
    let Some(file_path) = event["tool_input"]["file_path"].as_str() else {
        return ExitCode::SUCCESS;
    };
    if let Ok(Some(answer)) = framework_check::hook_answer(root, Path::new(file_path)) {
        println!("{answer}");
    }
    ExitCode::SUCCESS
}

fn check(root: &Path) -> ExitCode {
    match framework_check::check(root) {
        Ok(problems) if problems.is_empty() => {
            println!("the framework files match {}", framework_check::LOCK);
            ExitCode::SUCCESS
        }
        Ok(problems) => {
            eprintln!("{}", framework_check::explained(&problems));
            ExitCode::FAILURE
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
