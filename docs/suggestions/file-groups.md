# File groups: suggestions

Ideas that were not part of the first iteration. None are planned.

- A CLI binary that parses `--group-config`, `--group-include` and
  `--group-exclude` into `GroupFlags` (for example with `clap`).
- Group colors and ordering hints in the config for the UI.
- Group-level review state: a per-group summary of changed files, functions
  and lines.
- Presets for common stacks (Elixir/Phoenix, Rust workspaces) as starting
  config files.
- Writing the UI-edited `GroupConfig` back to `.code_reviewer.toml`.
- A `groups.local.toml` for personal groups that are not committed.
- Matching on module and function names, not only file paths.

## Code footer

- Syntax highlighting of the function code in the footer.
- Collapse long runs of unchanged lines inside a function, with a "show N
  lines" row.
- Side by side layout as an option next to the unified one.
- Load a file's code only when its ledger scrolls into view.
