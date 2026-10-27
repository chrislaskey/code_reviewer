use std::collections::HashSet;

use comrak::nodes::{AstNode, NodeValue};
use comrak::{format_html, parse_document, Arena, Options};

use super::function_diff::{DiffLine, DiffLineKind};

const MARKDOWN_EXTENSIONS: [&str; 3] = ["md", "markdown", "mkd"];

const SOURCEPOS_ATTRIBUTE: &str = " data-sourcepos=\"";

/// The attribute put on an HTML block that holds added lines. The frontend styles it.
pub const ADDED_ATTRIBUTE: &str = " data-added=\"\"";

/// True when the path names a Markdown file, judged by its extension.
pub fn is_markdown_path(path: &str) -> bool {
    path.rsplit_once('.').is_some_and(|(_, extension)| {
        MARKDOWN_EXTENSIONS.contains(&extension.to_lowercase().as_str())
    })
}

fn markdown_options() -> Options<'static> {
    let mut options = Options::default();
    options.extension.table = true;
    options.extension.strikethrough = true;
    options.extension.tasklist = true;
    options.extension.autolink = true;
    options.extension.footnotes = true;
    options.extension.alerts = true;
    options.render.gfm_quirks = true;
    options
}

/// Render GitHub flavored Markdown as HTML. Raw HTML in the text is left out,
/// so the result is safe to show on a page.
pub fn render_markdown_html(markdown: &str) -> String {
    render_markdown_html_marking_added_lines(markdown, &HashSet::new())
}

/// Render Markdown as HTML and mark the blocks that hold added lines with
/// [`ADDED_ATTRIBUTE`]. `added_lines` are 1-based line numbers of `markdown`.
///
/// A block made only of added lines is marked as a whole. A block that mixes
/// added and existing lines is not marked itself; the blocks inside it are
/// judged on their own. A block with no blocks inside is marked when any of
/// its lines is added.
pub fn render_markdown_html_marking_added_lines(
    markdown: &str,
    added_lines: &HashSet<usize>,
) -> String {
    let mut options = markdown_options();
    options.render.sourcepos = true;

    let arena = Arena::new();
    let root = parse_document(&arena, markdown, &options);
    let mut marked_positions = HashSet::new();
    collect_added_block_positions(root, added_lines, &mut marked_positions);

    let mut html = String::new();
    // Writing into a String cannot fail, so an error here is unreachable.
    if format_html(root, &options, &mut html).is_err() {
        html.clear();
    }
    replace_sourcepos_attributes(&html, &mut marked_positions)
}

fn collect_added_block_positions<'a>(
    parent: &'a AstNode<'a>,
    added_lines: &HashSet<usize>,
    marked_positions: &mut HashSet<String>,
) {
    for node in parent.children() {
        let (is_block, start_line, end_line) = {
            let data = node.data();
            (
                data.value.block(),
                data.sourcepos.start.line,
                data.sourcepos.end.line,
            )
        };
        if !is_block {
            continue;
        }
        let mut lines = start_line..=end_line;
        let all_added = lines.clone().all(|line| added_lines.contains(&line));
        let any_added = lines.any(|line| added_lines.contains(&line));
        let holds_blocks = node.children().any(|child| child.data().value.block());

        if holds_blocks && !all_added {
            collect_added_block_positions(node, added_lines, marked_positions);
        } else if any_added {
            let rendered = block_rendered_as_tag(node);
            marked_positions.insert(rendered.data().sourcepos.to_string());
        }
    }
}

/// A paragraph in a tight list has no tag of its own, so its list item carries the mark.
fn block_rendered_as_tag<'a>(node: &'a AstNode<'a>) -> &'a AstNode<'a> {
    let is_paragraph = matches!(node.data().value, NodeValue::Paragraph);
    let in_tight_list = node
        .parent()
        .and_then(|item| item.parent())
        .is_some_and(|list| match list.data().value {
            NodeValue::List(list) => list.tight,
            _ => false,
        });
    match node.parent() {
        Some(item) if is_paragraph && in_tight_list => item,
        _ => node,
    }
}

/// Turn each `data-sourcepos` attribute into [`ADDED_ATTRIBUTE`] when its position
/// was marked, and drop it otherwise. A position is used once, on the outermost tag.
fn replace_sourcepos_attributes(html: &str, marked_positions: &mut HashSet<String>) -> String {
    let mut output = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.find(SOURCEPOS_ATTRIBUTE) {
        output.push_str(&rest[..start]);
        let value_start = start + SOURCEPOS_ATTRIBUTE.len();
        let Some(value_length) = rest[value_start..].find('"') else {
            break;
        };
        let position = &rest[value_start..value_start + value_length];
        if marked_positions.remove(position) {
            output.push_str(ADDED_ATTRIBUTE);
        }
        rest = &rest[value_start + value_length + 1..];
    }
    output.push_str(rest);
    output
}

/// True when lines were left out between the two, judged by their line numbers.
fn has_gap_between(previous: &DiffLine, line: &DiffLine) -> bool {
    let jumped = |before: Option<usize>, after: Option<usize>| matches!((before, after), (Some(before), Some(after)) if after > before + 1);
    jumped(previous.old_number, line.old_number) || jumped(previous.new_number, line.new_number)
}

/// The new version of each stretch of changed lines, rendered as HTML with the
/// added blocks marked. Removed lines are left out. A stretch with no new lines
/// gives no HTML.
pub fn render_new_version_hunks(lines: &[DiffLine]) -> Vec<String> {
    let mut hunks: Vec<Vec<&DiffLine>> = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let starts_new_hunk = index == 0 || has_gap_between(&lines[index - 1], line);
        if starts_new_hunk {
            hunks.push(Vec::new());
        }
        if line.kind != DiffLineKind::Removed {
            if let Some(hunk) = hunks.last_mut() {
                hunk.push(line);
            }
        }
    }
    hunks
        .iter()
        .filter(|hunk| !hunk.is_empty())
        .map(|hunk| render_hunk(hunk))
        .collect()
}

fn render_hunk(lines: &[&DiffLine]) -> String {
    let text: Vec<&str> = lines.iter().map(|line| line.text.as_str()).collect();
    let added_lines: HashSet<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.kind == DiffLineKind::Added)
        .map(|(index, _)| index + 1)
        .collect();
    render_markdown_html_marking_added_lines(&text.join("\n"), &added_lines)
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
    fn test_nothing_is_marked_without_added_lines() {
        let html = render_markdown_html("# Title\n\n- one\n- two\n");

        assert!(!html.contains("data-added"));
        assert!(!html.contains("data-sourcepos"));
    }

    #[test]
    fn test_a_block_made_of_added_lines_is_marked_as_a_whole() {
        let markdown = "# Title\n\n- one\n- two\n\nAfter *all*.\n";
        let added_lines = HashSet::from([3, 4, 5, 6]);

        let html = render_markdown_html_marking_added_lines(markdown, &added_lines);

        assert!(html.contains("<h1>Title</h1>"));
        assert!(html.contains("<ul data-added=\"\">"));
        assert!(html.contains("<p data-added=\"\">After <em>all</em>.</p>"));
        assert!(!html.contains("<li data-added"));
        assert!(!html.contains("data-sourcepos"));
    }

    #[test]
    fn test_an_added_item_in_a_tight_list_is_marked_alone() {
        let markdown = "- one\n- two\n- three\n";
        let added_lines = HashSet::from([2]);

        let html = render_markdown_html_marking_added_lines(markdown, &added_lines);

        assert!(html.contains("<ul>"));
        assert_eq!(html.matches("data-added").count(), 1);
        assert!(html.contains("<li data-added=\"\">two</li>"));
    }

    #[test]
    fn test_a_paragraph_with_one_added_line_is_marked() {
        let markdown = "first line\nsecond line\n\nother\n";
        let added_lines = HashSet::from([2]);

        let html = render_markdown_html_marking_added_lines(markdown, &added_lines);

        assert!(html.contains("<p data-added=\"\">first line\nsecond line</p>"));
        assert!(html.contains("<p>other</p>"));
    }

    #[test]
    fn test_an_added_row_in_a_table_is_marked() {
        let markdown = "| a | b |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |\n";
        let added_lines = HashSet::from([4]);

        let html = render_markdown_html_marking_added_lines(markdown, &added_lines);

        assert_eq!(html.matches("data-added").count(), 1);
        assert!(html.contains("<tr data-added=\"\">"));
    }

    #[test]
    fn test_hunks_show_the_new_version_with_added_blocks_marked() {
        let old: String = (1..=20).map(|n| format!("line {n}\n\n")).collect();
        let new = old
            .replace("line 2\n", "# changed\n")
            .replace("line 18\n", "later\n");

        let hunks = render_new_version_hunks(&diff_texts(&old, &new));

        assert_eq!(hunks.len(), 2);
        assert!(hunks[0].contains("<h1 data-added=\"\">changed</h1>"));
        assert!(hunks[0].contains("<p>line 1</p>"));
        assert!(!hunks[0].contains("line 2"));
        assert!(hunks[1].contains("<p data-added=\"\">later</p>"));
    }

    #[test]
    fn test_a_text_with_only_removed_lines_has_no_hunks() {
        assert!(render_new_version_hunks(&diff_texts("one\ntwo\n", "")).is_empty());
    }
}
