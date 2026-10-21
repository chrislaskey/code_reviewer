# Markdown view

Ideas that came up while adding the HTML | Raw toggle for Markdown files.
Not planned yet.

- Mark the changed blocks in the HTML view (tint paragraphs, list items and
  rows that hold added lines). Today the HTML shows the new text without marks.
  comrak's `render.sourcepos` gives each block its line range to match against.
- Show removed lines in the HTML view, for example as struck-through blocks.
- Relative links and images: resolve them against the repository.
- Highlight code in fenced blocks (comrak's `syntect` feature, or highlight.js).
- Render the whole file, not only the changed stretches.
