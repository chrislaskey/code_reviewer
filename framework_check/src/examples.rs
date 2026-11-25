//! The examples' names in the framework's code: each is a line an app
//! deleting the example would break, so none is allowed, see
//! `docs/design/examples-delete.md`. The names come from the `examples`
//! list of `framework.toml`: `core/src/app/posts.rs` is `posts`,
//! `core/src/data/app_files/**` is `app_files`. A name is looked for in
//! the language of its example: those under `svelte/` in the pages, the
//! rest in Rust.

use std::path::Path;

use crate::{CheckError, Manifest, framework_files};

/// A line of a framework file that names an example.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExampleNamed {
    pub path: String,
    pub line: usize,
    pub name: String,
}

impl std::fmt::Display for ExampleNamed {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}:{} names {}", self.path, self.line, self.name)
    }
}

/// The names of the examples in the language, each glob's last part
/// without wildcards or an extension, in the order listed.
fn list_names(manifest: &Manifest, language: Language) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for glob in &manifest.examples {
        if Language::of_example(glob) != language {
            continue;
        }
        let path = glob.strip_suffix("/**").unwrap_or(glob);
        let part = path.rsplit('/').next().unwrap_or(path);
        if part.contains('*') {
            continue;
        }
        let name = part
            .split_once('.')
            .map_or(part, |(stem, _)| stem)
            .to_string();
        if !names.contains(&name) {
            names.push(name);
        }
    }
    names
}

/// Every line of the framework's Rust, JavaScript and Svelte that names
/// an example, outside comments and tests.
pub fn list_named_in_framework(
    root: &Path,
    manifest: &Manifest,
) -> Result<Vec<ExampleNamed>, CheckError> {
    let rust_names = list_names(manifest, Language::Rust);
    let pages_names = list_names(manifest, Language::Pages);
    let mut named = Vec::new();
    for (path, file) in framework_files(root, manifest)? {
        let Some(language) = Language::of(&path) else {
            continue;
        };
        if language == Language::Rust && is_tests_file(&path) {
            continue;
        }
        let text = std::fs::read_to_string(&file).map_err(|source| CheckError::Read {
            path: file.clone(),
            source,
        })?;
        let names = match language {
            Language::Rust => &rust_names,
            Language::Pages => &pages_names,
        };
        for (line, name) in names_in(&text, names, language) {
            named.push(ExampleNamed {
                path: path.clone(),
                line,
                name,
            });
        }
    }
    Ok(named)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Language {
    Rust,
    Pages,
}

impl Language {
    fn of(path: &str) -> Option<Language> {
        if path.ends_with(".rs") {
            Some(Language::Rust)
        } else if path.ends_with(".js") || path.ends_with(".svelte") {
            Some(Language::Pages)
        } else {
            None
        }
    }

    fn of_example(glob: &str) -> Language {
        if glob.starts_with("svelte/") {
            Language::Pages
        } else {
            Language::Rust
        }
    }
}

fn is_tests_file(path: &str) -> bool {
    path.split('/')
        .any(|part| part == "tests" || part == "tests.rs")
}

/// The names in the text, by line number from 1, outside comments and,
/// in Rust, before the tests. In Rust a name as a path, `posts::list`,
/// `app::posts` or in a `use`, a table, `app_posts`, or a route,
/// `"/api/posts"`. In the pages a name joined to another word,
/// `postsIndexUrl` or `PostsForm`, a component, `FileImage`, or a part
/// of a path, `"../posts/PostsForm.svelte"`.
fn names_in(text: &str, names: &[String], language: Language) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    let mut in_comment: Option<&str> = None;
    let mut in_use = false;
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        if let Some(end) = in_comment {
            if trimmed.contains(end) {
                in_comment = None;
            }
            continue;
        }
        if language == Language::Rust && trimmed.starts_with("#[cfg(") && trimmed.contains("test") {
            break;
        }
        if trimmed.starts_with("//") || trimmed.starts_with('*') {
            continue;
        }
        if let Some((_, end)) = [("/*", "*/"), ("<!--", "-->")]
            .into_iter()
            .find(|(start, _)| trimmed.starts_with(start))
        {
            if !trimmed.contains(end) {
                in_comment = Some(end);
            }
            continue;
        }
        if language == Language::Rust
            && (trimmed.starts_with("use ") || trimmed.starts_with("pub use "))
        {
            in_use = true;
        }
        for name in names {
            let named = match language {
                Language::Rust => rust_names(line, name, in_use),
                Language::Pages => pages_name(line, name),
            };
            if named {
                found.push((index + 1, name.clone()));
            }
        }
        if in_use && line.contains(';') {
            in_use = false;
        }
    }
    found
}

fn rust_names(line: &str, name: &str, in_use: bool) -> bool {
    identifiers(line).any(|(start, identifier)| {
        let before = &line[..start];
        let after = &line[start + identifier.len()..];
        identifier == name
            && (name.starts_with("app_")
                || in_use
                || before.ends_with("::")
                || after.starts_with("::")
                || before.ends_with("/api/"))
    })
}

fn pages_name(line: &str, name: &str) -> bool {
    let camel = camel_case(name);
    let pascal = pascal_case(name);
    identifiers(line).any(|(start, identifier)| {
        let before = &line[..start];
        let after = &line[start + identifier.len()..];
        let in_path = identifier == name
            && before.ends_with('/')
            && (after.starts_with('/') || after.starts_with('.') || after.starts_with('"'));
        let component = identifier == name && name != name.to_lowercase();
        in_path
            || component
            || [camel.as_str(), pascal.as_str()]
                .iter()
                .any(|form| joined_to_a_word(identifier, form))
    })
}

/// The identifier is the form followed by another word, as
/// `postsIndexUrl` is `posts` and `IndexUrl`.
fn joined_to_a_word(identifier: &str, form: &str) -> bool {
    identifier
        .strip_prefix(form)
        .and_then(|rest| rest.chars().next())
        .is_some_and(char::is_uppercase)
}

/// Each identifier of the line, with where it starts.
fn identifiers(line: &str) -> impl Iterator<Item = (usize, &str)> {
    let is_part = |character: char| character.is_alphanumeric() || character == '_';
    line.char_indices()
        .filter(move |(start, character)| {
            is_part(*character) && !line[..*start].chars().next_back().is_some_and(is_part)
        })
        .map(move |(start, _)| {
            let length = line[start..]
                .find(|character: char| !is_part(character))
                .unwrap_or(line.len() - start);
            (start, &line[start..start + length])
        })
}

fn camel_case(name: &str) -> String {
    let mut words = name.split('_');
    let first = words.next().unwrap_or_default().to_string();
    words.fold(first, |camel, word| camel + &pascal_case(word))
}

fn pascal_case(name: &str) -> String {
    name.split('_')
        .map(|word| {
            let mut characters = word.chars();
            match characters.next() {
                Some(first) => first.to_uppercase().chain(characters).collect(),
                None => String::new(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names() -> Vec<String> {
        vec!["posts".to_string(), "app_posts".to_string()]
    }

    #[test]
    fn framework_check_examples_list_names_takes_each_globs_last_plain_part_by_language() {
        let manifest = Manifest {
            examples: vec![
                "core/src/app/posts.rs".to_string(),
                "core/src/app/posts/**".to_string(),
                "core/src/data/app_posts/**".to_string(),
                "core/migrations/*/*_app_posts_create.sql".to_string(),
                "svelte/src/posts/**".to_string(),
                "svelte/src/lib/FileImage.svelte".to_string(),
            ],
            ..Manifest::default()
        };
        assert_eq!(
            list_names(&manifest, Language::Rust),
            ["posts", "app_posts"]
        );
        assert_eq!(
            list_names(&manifest, Language::Pages),
            ["posts", "FileImage"]
        );
    }

    #[test]
    fn framework_check_examples_names_in_rust_finds_paths_tables_and_routes_outside_comments_and_tests()
     {
        let text = "use crate::app::posts;\n\
                    use crate::app::{\n\
                        communities, posts,\n\
                    };\n\
                    // posts in a comment\n\
                    let posts = posts::list();\n\
                    let posts_left = posts.len();\n\
                    route(\"/api/posts\");\n\
                    sqlx::query(\"SELECT * FROM app_posts\");\n\
                    #[cfg(test)]\n\
                    use crate::app::posts;\n";
        assert_eq!(
            names_in(text, &names(), Language::Rust),
            [
                (1, "posts".to_string()),
                (3, "posts".to_string()),
                (6, "posts".to_string()),
                (8, "posts".to_string()),
                (9, "app_posts".to_string()),
            ]
        );
    }

    #[test]
    fn framework_check_examples_names_in_pages_finds_joined_names_components_and_paths() {
        let text = "<!-- About posts,\n\
                    posts again -->\n\
                    import { postsIndexUrl } from \"../lib/routes.js\";\n\
                    import Form from \"../posts/components/Form.svelte\";\n\
                    import { FileImage } from \"../lib/index.js\";\n\
                    let posts = items.posts;\n\
                    let postscript = 1;\n\
                    /** posts */\n";
        let mut names = names();
        names.push("FileImage".to_string());
        assert_eq!(
            names_in(text, &names, Language::Pages),
            [
                (3, "posts".to_string()),
                (4, "posts".to_string()),
                (5, "FileImage".to_string()),
            ]
        );
    }
}
