use comrak::{markdown_to_html, Options};

use super::function_diff::{DiffLine, DiffLineKind};

const MARKDOWN_EXTENSIONS: [&str; 3] = ["md", "markdown", "mkd"];

/// True when the path names a Markdown file, judged by its extension.
pub fn is_markdown_path(path: &str) -> bool {
    path.rsplit_once('.').is_some_and(|(_, extension)| {
        MARKDOWN_EXTENSIONS.contains(&extension.to_lowercase().as_str())
    })
}

/// Render GitHub flavored Markdown as HTML. Raw HTML in the text is left out,
/// so the result is safe to show on a page.
pub fn render_markdown_html(markdown: &str) -> String {
    let mut options = Options::default();
    options.extension.table = true;
    options.extension.strikethrough = true;
    options.extension.tasklist = true;
    options.extension.autolink = true;
    options.extension.footnotes = true;
    options.extension.alerts = true;
    options.render.gfm_quirks = true;
    markdown_to_html(markdown, &options)
}

/// True when lines were left out between the two, judged by their line numbers.
fn has_gap_between(previous: &DiffLine, line: &DiffLine) -> bool {
    let jumped = |before: Option<usize>, after: Option<usize>| matches!((before, after), (Some(before), Some(after)) if after > before + 1);
    jumped(previous.old_number, line.old_number) || jumped(previous.new_number, line.new_number)
}

/// The new version of each stretch of changed lines, rendered as HTML.
/// Removed lines are left out. A stretch with no new lines gives no HTML.
pub fn render_new_version_hunks(lines: &[DiffLine]) -> Vec<String> {
    let mut hunks: Vec<Vec<&str>> = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let starts_new_hunk = index == 0 || has_gap_between(&lines[index - 1], line);
        if starts_new_hunk {
            hunks.push(Vec::new());
        }
        if line.kind != DiffLineKind::Removed {
            if let Some(hunk) = hunks.last_mut() {
                hunk.push(&line.text);
            }
        }
    }
    hunks
        .iter()
        .filter(|hunk| !hunk.is_empty())
        .map(|hunk| render_markdown_html(&hunk.join("\n")))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::file_diff::diff_texts;

    #[test]
    fn test_markdown_paths_are_found_by_extension() {
        assert!(is_markdown_path("README.md"));
        assert!(is_markdown_path("docs/Guide.MARKDOWN"));
        assert!(!is_markdown_path("lib/app.ex"));
        assert!(!is_markdown_path("md"));
    }

    #[test]
    fn test_github_flavored_markdown_is_rendered() {
        let html =
            render_markdown_html("| a | b |\n|---|---|\n| 1 | 2 |\n\n- [x] done\n\n~~gone~~\n");

        assert!(html.contains("<table>"));
        assert!(html.contains(r#"type="checkbox""#));
        assert!(html.contains("<del>gone</del>"));
    }

    #[test]
    fn test_raw_html_and_script_links_are_not_rendered() {
        let html = render_markdown_html("<script>alert(1)</script>\n\n[x](javascript:alert(1))\n");

        assert!(!html.contains("<script"));
        assert!(!html.contains("javascript:"));
    }

    #[test]
    fn test_hunks_show_the_new_version_without_removed_lines() {
        let old: String = (1..=20).map(|n| format!("line {n}\n")).collect();
        let new = old
            .replace("line 2\n", "# changed\n")
            .replace("line 18\n", "later\n");

        let hunks = render_new_version_hunks(&diff_texts(&old, &new));

        assert_eq!(hunks.len(), 2);
        assert!(hunks[0].contains("changed"));
        assert!(!hunks[0].contains("line 2"));
        assert!(hunks[1].contains("later"));
    }

    #[test]
    fn test_a_text_with_only_removed_lines_has_no_hunks() {
        assert!(render_new_version_hunks(&diff_texts("one\ntwo\n", "")).is_empty());
    }
}
