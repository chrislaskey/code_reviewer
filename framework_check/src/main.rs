//! `cargo run -p framework_check -- check|lock|hook|classify`: checks
//! the template's own files against `framework.lock`, rewrites the lock,
//! answers a Claude Code `PreToolUse` hook about one edit, or prints the
//! kind of each path given, from the repository's root, see the library.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    let Some(root) = repository_root() else {
        eprintln!("no {} here or in a folder above", framework_check::MANIFEST);
        return ExitCode::FAILURE;
    };
    let root = root.as_path();
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
        Some("classify") => classify(root, std::env::args().skip(2).collect()),
        Some(other) => {
            eprintln!("unknown command {other}; commands: check, lock, hook, classify");
            ExitCode::FAILURE
        }
    }
}

/// The nearest folder at or above the current one that holds
/// `framework.toml`: the checkout the command was run in, wherever the
/// binary was built.
fn repository_root() -> Option<PathBuf> {
    let current = std::env::current_dir().ok()?;
    current
        .ancestors()
        .find(|folder| folder.join(framework_check::MANIFEST).is_file())
        .map(Path::to_path_buf)
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

/// Prints `<kind> <path>` for each path, from the root: what
/// `bin/framework` asks.
fn classify(root: &Path, paths: Vec<String>) -> ExitCode {
    let manifest = match framework_check::manifest(root) {
        Ok(manifest) => manifest,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    for path in paths {
        match framework_check::kind_of(&manifest, &path) {
            Ok(kind) => println!("{kind} {path}"),
            Err(error) => {
                eprintln!("{error}");
                return ExitCode::FAILURE;
            }
        }
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
