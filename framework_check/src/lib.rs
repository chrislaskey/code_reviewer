//! The template's own files in an app built from it: `framework.toml`
//! names them, `framework.lock` holds the SHA-256 of each as the template
//! last shipped it, and `check` finds every one that differs, so a
//! change to the template's code is a choice someone made, not an
//! accident a later merge finds. See `docs/design/template-and-apps.md`.
//!
//! A file differing from the lock is fine once `framework.toml` declares
//! it a divergence, with why. In the template itself the lock is
//! rewritten with `lock` whenever a framework file changes.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use glob::{MatchOptions, Pattern};
use ignore::WalkBuilder;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use thiserror::Error;

/// The manifest at the root of the repository.
pub const MANIFEST: &str = "framework.toml";
/// The hashes at the root of the repository.
pub const LOCK: &str = "framework.lock";
/// Folders never walked: builds, dependencies, git's own. Besides these,
/// what the repository's `.gitignore` files ignore is never hashed, such
/// as what a build writes beside the template's files.
const SKIPPED_FOLDERS: &[&str] = &[".git", "target", "node_modules", "dist", ".worktrees"];
/// Files never hashed: what an operating system leaves in a folder.
const SKIPPED_FILES: &[&str] = &[".DS_Store"];

/// `framework.toml`: which files are the template's, which every app
/// edits, which are examples to replace, and the template's files this
/// app changed on purpose.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Manifest {
    /// The template's machinery, as globs from the root.
    #[serde(default)]
    pub framework: Vec<String>,
    /// Files every app edits to add a feature, never checked; they take
    /// precedence over `framework`.
    #[serde(default)]
    pub seams: Vec<String>,
    /// Features that show the patterns, the app's to replace, never
    /// checked; they take precedence over `framework`.
    #[serde(default)]
    pub examples: Vec<String>,
    #[serde(default)]
    pub divergence: Vec<Divergence>,
}

/// A framework file this app changed on purpose.
#[derive(Clone, Debug, Deserialize)]
pub struct Divergence {
    pub path: String,
    pub reason: String,
}

/// A way the framework files differ from the lock that nobody declared.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Problem {
    /// Changed since the template shipped it.
    Changed(String),
    /// In the lock, gone from the repository.
    Removed(String),
    /// A framework file the lock does not know.
    Added(String),
    /// Declared a divergence, but the same as the lock again.
    DivergenceNotNeeded(String),
    /// Declared a divergence without saying why.
    DivergenceWithoutReason(String),
}

impl fmt::Display for Problem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Problem::Changed(path) => write!(formatter, "changed: {path}"),
            Problem::Removed(path) => write!(formatter, "removed: {path}"),
            Problem::Added(path) => write!(formatter, "added: {path}"),
            Problem::DivergenceNotNeeded(path) => write!(
                formatter,
                "declared a divergence but the same as the template again, remove it: {path}"
            ),
            Problem::DivergenceWithoutReason(path) => {
                write!(formatter, "declared a divergence without a reason: {path}")
            }
        }
    }
}

#[derive(Debug, Error)]
pub enum CheckError {
    #[error("could not read {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("could not write {path}: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("could not walk the repository: {0}")]
    Walk(#[from] ignore::Error),
    #[error("{MANIFEST} is not valid: {0}")]
    Manifest(#[from] toml::de::Error),
    #[error("{MANIFEST} has a glob that is not valid, {glob}: {source}")]
    Glob {
        glob: String,
        source: glob::PatternError,
    },
    #[error("{LOCK} line {line} is not `<sha256>  <path>`")]
    Lock { line: usize },
}

/// Which of `framework.toml`'s kinds a file is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Framework,
    Seam,
    Example,
    /// Named nowhere: the app's own.
    App,
}

impl fmt::Display for Kind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Kind::Framework => "framework",
            Kind::Seam => "seam",
            Kind::Example => "example",
            Kind::App => "app",
        })
    }
}

/// The kind of the file at the path, from the root. Seams and examples
/// take precedence over framework.
pub fn kind_of(manifest: &Manifest, path: &str) -> Result<Kind, CheckError> {
    Ok(if matches_any(&patterns(&manifest.seams)?, path) {
        Kind::Seam
    } else if matches_any(&patterns(&manifest.examples)?, path) {
        Kind::Example
    } else if matches_any(&patterns(&manifest.framework)?, path) {
        Kind::Framework
    } else {
        Kind::App
    })
}

/// Whether the path, from the root, is a framework file that no
/// divergence declares: what an edit to it should stop and think about.
pub fn is_undeclared_framework_file(manifest: &Manifest, path: &str) -> Result<bool, CheckError> {
    let declared = manifest
        .divergence
        .iter()
        .any(|divergence| divergence.path == path);
    Ok(kind_of(manifest, path)? == Kind::Framework && !declared)
}

/// What a Claude Code `PreToolUse` hook answers for an edit of the file,
/// given the absolute path the tool names: a note for the model and the
/// person when it is a framework file, nothing otherwise. Never blocks.
pub fn hook_answer(root: &Path, file_path: &Path) -> Result<Option<String>, CheckError> {
    let Some(path) = relative_path(root, file_path) else {
        return Ok(None);
    };
    if !is_undeclared_framework_file(&manifest(root)?, &path)? {
        return Ok(None);
    }
    let context = format!(
        "{path} is template code (framework.toml), checked against framework.lock. \
         Before changing it, prefer extending it from the app's own files, or changing \
         it in the template and merging the template here. If it must change in this \
         app, declare it in framework.toml as a [[divergence]] with the reason, and say \
         so in the summary. In the template itself, run \
         `cargo run -p framework_check -- lock` after changing it."
    );
    let answer = serde_json::json!({
        "systemMessage": format!("Editing template code: {path}"),
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "additionalContext": context,
        },
    });
    Ok(Some(answer.to_string()))
}

/// Reads `framework.toml` at the root.
pub fn manifest(root: &Path) -> Result<Manifest, CheckError> {
    let path = root.join(MANIFEST);
    let text =
        std::fs::read_to_string(&path).map_err(|source| CheckError::Read { path, source })?;
    Ok(toml::from_str(&text)?)
}

/// Every framework file under the root, by its path from the root with
/// `/` between folders, with its SHA-256.
pub fn framework_hashes(
    root: &Path,
    manifest: &Manifest,
) -> Result<BTreeMap<String, String>, CheckError> {
    let framework = patterns(&manifest.framework)?;
    let exceptions = patterns(manifest.seams.iter().chain(&manifest.examples))?;
    let mut hashes = BTreeMap::new();
    // The repository's own .gitignore files alone, not a person's global
    // ones, so every machine hashes the same files
    let walk = WalkBuilder::new(root)
        .hidden(false)
        .git_global(false)
        .git_exclude(false)
        .require_git(false)
        .filter_entry(|entry| {
            !(entry.file_type().is_some_and(|kind| kind.is_dir())
                && SKIPPED_FOLDERS.contains(&entry.file_name().to_string_lossy().as_ref()))
        })
        .build();
    for entry in walk {
        let entry = entry?;
        if !entry.file_type().is_some_and(|kind| kind.is_file())
            || SKIPPED_FILES.contains(&entry.file_name().to_string_lossy().as_ref())
        {
            continue;
        }
        let Some(path) = relative_path(root, entry.path()) else {
            continue;
        };
        if matches_any(&framework, &path) && !matches_any(&exceptions, &path) {
            hashes.insert(path, sha256_of(entry.path())?);
        }
    }
    Ok(hashes)
}

/// Reads `framework.lock` at the root: one `<sha256>  <path>` a line.
pub fn read_lock(root: &Path) -> Result<BTreeMap<String, String>, CheckError> {
    let path = root.join(LOCK);
    let text =
        std::fs::read_to_string(&path).map_err(|source| CheckError::Read { path, source })?;
    let mut hashes = BTreeMap::new();
    for (index, line) in text.lines().enumerate() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (hash, path) = line
            .split_once("  ")
            .ok_or(CheckError::Lock { line: index + 1 })?;
        hashes.insert(path.to_string(), hash.to_string());
    }
    Ok(hashes)
}

/// Writes `framework.lock` at the root from the framework files as they
/// are now: what the template does whenever one of them changes.
pub fn write_lock(root: &Path) -> Result<usize, CheckError> {
    let hashes = framework_hashes(root, &manifest(root)?)?;
    let mut text = String::from(
        "# The SHA-256 of each framework file as the template ships it, see\n\
         # framework.toml. Written by `cargo run -p framework_check -- lock`.\n",
    );
    for (path, hash) in &hashes {
        text.push_str(&format!("{hash}  {path}\n"));
    }
    let path = root.join(LOCK);
    std::fs::write(&path, text).map_err(|source| CheckError::Write { path, source })?;
    Ok(hashes.len())
}

/// Every way the framework files under the root differ from the lock
/// that `framework.toml` does not declare; none when they all match.
pub fn check(root: &Path) -> Result<Vec<Problem>, CheckError> {
    let manifest = manifest(root)?;
    let now = framework_hashes(root, &manifest)?;
    let locked = read_lock(root)?;
    Ok(problems(&manifest, &now, &locked))
}

/// The differences between the files as they are and as locked, less
/// the declared divergences.
pub fn problems(
    manifest: &Manifest,
    now: &BTreeMap<String, String>,
    locked: &BTreeMap<String, String>,
) -> Vec<Problem> {
    let declared: BTreeMap<&str, &Divergence> = manifest
        .divergence
        .iter()
        .map(|divergence| (divergence.path.as_str(), divergence))
        .collect();
    let mut problems = Vec::new();
    for divergence in &manifest.divergence {
        if divergence.reason.trim().is_empty() {
            problems.push(Problem::DivergenceWithoutReason(divergence.path.clone()));
        }
    }
    for (path, hash) in locked {
        let differs = match now.get(path) {
            Some(current) if current == hash => None,
            Some(_) => Some(Problem::Changed(path.clone())),
            None => Some(Problem::Removed(path.clone())),
        };
        match (differs, declared.contains_key(path.as_str())) {
            (Some(problem), false) => problems.push(problem),
            (None, true) => problems.push(Problem::DivergenceNotNeeded(path.clone())),
            _ => {}
        }
    }
    for path in now.keys() {
        if !locked.contains_key(path) && !declared.contains_key(path.as_str()) {
            problems.push(Problem::Added(path.clone()));
        }
    }
    problems
}

/// The problems, with what to do about them, for a person reading a
/// failed check.
pub fn explained(problems: &[Problem]) -> String {
    let mut text = String::from(
        "These template files differ from framework.lock, see docs/design/template-and-apps.md:\n\n",
    );
    for problem in problems {
        text.push_str(&format!("  {problem}\n"));
    }
    text.push_str(
        "\nFor each, either:\n\
         - extend it from the app's own files instead, and undo the change;\n\
         - change it in the template, then merge the template here;\n\
         - or keep it, declared in framework.toml as\n\
         \n    [[divergence]]\n    path = \"<path>\"\n    reason = \"<why>\"\n\
         \nIn the template itself, run `cargo run -p framework_check -- lock`.",
    );
    text
}

fn patterns<'a>(globs: impl IntoIterator<Item = &'a String>) -> Result<Vec<Pattern>, CheckError> {
    globs
        .into_iter()
        .map(|glob| {
            Pattern::new(glob).map_err(|source| CheckError::Glob {
                glob: glob.clone(),
                source,
            })
        })
        .collect()
}

fn matches_any(patterns: &[Pattern], path: &str) -> bool {
    let options = MatchOptions {
        case_sensitive: true,
        require_literal_separator: true,
        require_literal_leading_dot: false,
    };
    patterns
        .iter()
        .any(|pattern| pattern.matches_with(path, options))
}

/// The path from the root, folders joined by `/` on every platform.
fn relative_path(root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?;
    let parts: Option<Vec<&str>> = relative
        .components()
        .map(|component| component.as_os_str().to_str())
        .collect();
    Some(parts?.join("/"))
}

fn sha256_of(path: &Path) -> Result<String, CheckError> {
    let bytes = std::fs::read(path).map_err(|source| CheckError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hashes(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
        entries
            .iter()
            .map(|(path, hash)| (path.to_string(), hash.to_string()))
            .collect()
    }

    fn diverged(path: &str, reason: &str) -> Manifest {
        Manifest {
            divergence: vec![Divergence {
                path: path.to_string(),
                reason: reason.to_string(),
            }],
            ..Manifest::default()
        }
    }

    #[test]
    fn framework_check_framework_hashes_skip_what_gitignore_ignores() {
        let root = tempfile::tempdir().expect("a folder");
        let plugin = root.path().join("plugins/kit");
        std::fs::create_dir_all(plugin.join("ios")).expect("folders");
        std::fs::write(plugin.join(".gitignore"), "/ios/Package.resolved\n").expect("written");
        std::fs::write(plugin.join("ios/Plugin.swift"), "plugin").expect("written");
        std::fs::write(plugin.join("ios/Package.resolved"), "resolved").expect("written");
        let manifest = Manifest {
            framework: vec!["plugins/**".to_string()],
            ..Manifest::default()
        };

        let hashed = framework_hashes(root.path(), &manifest).expect("hashed");

        let paths: Vec<&str> = hashed.keys().map(String::as_str).collect();
        assert_eq!(
            paths,
            ["plugins/kit/.gitignore", "plugins/kit/ios/Plugin.swift"]
        );
    }

    #[test]
    fn framework_check_problems_finds_changed_removed_and_added_files() {
        let locked = hashes(&[("a.rs", "1"), ("b.rs", "2")]);
        let now = hashes(&[("a.rs", "changed"), ("c.rs", "3")]);
        let problems = problems(&Manifest::default(), &now, &locked);
        assert_eq!(
            problems,
            [
                Problem::Changed("a.rs".to_string()),
                Problem::Removed("b.rs".to_string()),
                Problem::Added("c.rs".to_string()),
            ]
        );
    }

    #[test]
    fn framework_check_problems_accepts_a_declared_divergence_with_a_reason() {
        let locked = hashes(&[("a.rs", "1")]);
        let now = hashes(&[("a.rs", "changed")]);
        assert!(problems(&diverged("a.rs", "tuned for us"), &now, &locked).is_empty());
        assert_eq!(
            problems(&diverged("a.rs", " "), &now, &locked),
            [Problem::DivergenceWithoutReason("a.rs".to_string())]
        );
    }

    #[test]
    fn framework_check_problems_asks_to_remove_a_divergence_no_longer_needed() {
        let locked = hashes(&[("a.rs", "1")]);
        assert_eq!(
            problems(&diverged("a.rs", "tuned for us"), &locked, &locked),
            [Problem::DivergenceNotNeeded("a.rs".to_string())]
        );
    }

    #[test]
    fn framework_check_is_undeclared_framework_file_skips_seams_examples_and_divergences() {
        let manifest = Manifest {
            framework: vec!["core/src/**".to_string()],
            seams: vec!["core/src/app.rs".to_string()],
            examples: vec!["core/src/app/posts.rs".to_string()],
            divergence: vec![Divergence {
                path: "core/src/sync.rs".to_string(),
                reason: "tuned for us".to_string(),
            }],
        };
        let undeclared = |path| is_undeclared_framework_file(&manifest, path).expect("the globs");
        assert!(undeclared("core/src/jobs.rs"));
        assert!(!undeclared("core/src/app.rs"));
        assert!(!undeclared("core/src/app/posts.rs"));
        assert!(!undeclared("core/src/sync.rs"));
        assert!(!undeclared("svelte/src/home/HomePage.svelte"));
    }

    #[test]
    fn framework_check_kind_of_lets_seams_and_examples_win_over_framework() {
        let manifest = Manifest {
            framework: vec!["core/src/**".to_string()],
            seams: vec!["core/src/app.rs".to_string()],
            examples: vec!["core/src/app/posts.rs".to_string()],
            ..Manifest::default()
        };
        let kind = |path| kind_of(&manifest, path).expect("the globs");
        assert_eq!(kind("core/src/jobs.rs"), Kind::Framework);
        assert_eq!(kind("core/src/app.rs"), Kind::Seam);
        assert_eq!(kind("core/src/app/posts.rs"), Kind::Example);
        assert_eq!(kind("svelte/src/home/HomePage.svelte"), Kind::App);
    }

    #[test]
    fn framework_check_matches_any_takes_double_stars_across_folders_and_stars_within_one() {
        let patterns = patterns(&["core/src/sync/**".to_string(), "core/*.rs".to_string()])
            .expect("the globs");
        assert!(matches_any(&patterns, "core/src/sync/bytes.rs"));
        assert!(matches_any(&patterns, "core/src/sync/deep/er.rs"));
        assert!(matches_any(&patterns, "core/build.rs"));
        assert!(!matches_any(&patterns, "core/src/lib.rs"));
    }
}
