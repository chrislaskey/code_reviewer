//! `cargo run -p framework_check -- check|lock`: checks the template's
//! own files against `framework.lock`, or rewrites the lock, from the
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
        Some(other) => {
            eprintln!("unknown command {other}; commands: check, lock");
            ExitCode::FAILURE
        }
    }
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
