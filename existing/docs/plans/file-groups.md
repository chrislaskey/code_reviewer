# File groups

Groups are named sets of files ("data", "web", "tests", "docs"). They let a
review be segmented by area of the app. A file can belong to many groups.
Groups are one flat list: there is no nesting.

## Phases

1. **Definition** (done): group model, config file, flags, pattern matching.
   Code in `core/src/groups/`, TS types in `frontend/src/api/types.ts`.
2. **UI/UX** (done). Two levels, **Groups > Functions**:
   - The **Groups view** is the file view one level up, with the same ledger and
     node renderers. Header is a group; content is its files; Outputs and
     Inputs are the files outside the group that its files call or are called by.
   - Clicking a file opens the **Functions view** (the view for a single file)
     for every file in that group.
   - A scrolling page, like the file tree on a GitHub pull request. The tree on
     the left is **All changes by group** with the groups nested under it. One
     click always means the same view:
     - **All changes by group** shows only the summary row, which lists every
       group. The review opens here.
     - A **group** shows the Groups view (a row per group), scrolled to that
       group, and lists the group's files under it in the tree.
     - A **file**, in the tree or in a row, shows the Functions view of its
       group, scrolled to that file.
   - Group rollups stay small: `2 files +30 −12`, with a warning sign when
     callers are stale. A group ledger header is just `3 files`, a file header
     `3 functions`. Stale inputs are drawn in the normal text colour.
   - Page: `/explorations/groups-flow` (hardcoded data, same component as the
     review page). Example data is in `frontend/src/explorations/groups_data.ts`,
     the row builder in `group_rows.ts`, the renderers in `group_views.tsx`.
     The earlier nested-group explorations were removed.
3. **Backend integration** (done), see "What is implemented".

## Decisions and options

Each decision lists the options, then the choice (marked **Chosen**).

### 1. Config file format

| Option | For | Against |
| --- | --- | --- |
| **TOML** | Comments, no indentation traps, globs read cleanly, native to Rust | |
| YAML | Compact | Indentation errors, `*` and `{}` in globs need quoting, implicit typing |
| JSON | The UI already speaks it | No comments, noisy for hand editing |

**Chosen: TOML** for the file. The same serde structs also read JSON, so the UI
sends `GroupConfig` as JSON with no second format. File name:
`.code_reviewer.toml` in the repo root (overridable, see flags). It is a
general settings file that only holds `[groups]` today. Unknown keys are errors
so typos do not silently disable a group. A config that still has `subgroups`
fails to parse for the same reason.

### 2. Flat groups, not nested

Nesting (`[groups.core.subgroups.data]`) was built first and removed. Parent
groups only rolled up their subgroups, had no rows or patterns of their own, and
every view needed rules for them (which row to scroll to, where files sit in the
tree). A flat list gives the same segmentation, since a file can be in many
groups: write a `core` group with its own patterns next to `data`.

A group is a name, an optional `label`, `include` patterns (at least one) and
optional `exclude` patterns. A file is in the group when any `include` matches
and no `exclude` matches. Table keys keep file order (`IndexMap`), so the UI
lists groups as written.

Group ids are the names. Names are slugs (`[A-Za-z0-9_-]+`) so they are safe in
flags and keys; `label` holds display text such as "Business logic".

### 2b. The implicit default group

Every file that no group holds goes to an implicit `ungrouped` group (label
"Default group"), listed last. The name `ungrouped` is reserved. The default
group only shows when it holds a changed file. The group view is always on:
with **no groups defined and no config file**, every file is in the default
group and the review page still shows the summary, groups and functions views.

### 4. Pattern language

| Option | Notes |
| --- | --- |
| **Glob, plus `regex:` prefix** | One field, glob by default, regex when asked |
| Glob only | Simplest, cannot say "ends in `_test` or `_spec`" without braces |
| Separate fields (`include_regex`) | Doubles the fields and the flags |
| Gitignore semantics | Familiar, but implicit "match anywhere" rules surprise |

**Chosen: glob with a `regex:` prefix**, matched against the repo-relative path
with `/` separators.

- Glob (via `globset`): matches the whole path. `*` stays within one
  directory, `**` crosses directories, `{a,b}` alternates. `*.md` is top-level
  only; write `**/*.md` for anywhere.
- `regex:<expression>` (via `regex`): matches anywhere in the path. Use `^`
  and `$` to anchor.
- A group holds a file when any `include` matches and no `exclude` matches.
- A pattern without a `/` is not special: `*.rs` matches top-level files only,
  never at any depth. Write `**/*.rs` for any depth. This may be revisited.
- Groups match file paths only, never modules or functions.

### 5. Command line flags

No CLI binary exists yet, so the flags are defined as `GroupFlags`, which a
future CLI parses into and the UI fills from a form.

| Option | Example |
| --- | --- |
| **One pattern per flag** | `--group-include data=lib/db/**` |
| One flag per group, patterns joined | `--group data=lib/db/**,!**/*_test.*` |
| Positional state | `--group data --include 'lib/db/**'` |

**Chosen: one pattern per flag.** No delimiter clashes with `{a,b}`, order does
not matter, and it maps to two plain string lists.

| Flag | Meaning |
| --- | --- |
| `--group-config <path>` | Use this file instead of the default. Must exist. |
| `--group-include <group>=<pattern>` | Repeatable. Add an include pattern. |
| `--group-exclude <group>=<pattern>` | Repeatable. Add an exclude pattern. |

`<group>` is a group name. The group is created when missing. Only the first `=` splits the flag. Flags **add to** the file; they
never remove or replace its patterns.

### 6. Where the UI fits

The UI sends a `GroupConfig` (and optionally `GroupFlags`) inside the analysis
request, the same way `module_rules` travels today. The backend then does
`load_group_config` (file + flags) and `compile`. The UI can also call
`GroupMatcher::list_groups` for the tree it draws.

## Example

```toml
# .code_reviewer.toml
[groups.data]
label = "Data"
include = ["core/src/storage/**", "**/repo/*.ex"]
exclude = ["**/*_test.*"]

[groups.web]
include = ["core/src/web/**"]

[groups.tests]
include = ["**/*_test.*", "**/tests/**", "regex:_(spec|test)\\."]

[groups.docs]
include = ["docs/**", "**/*.md"]
```

```sh
--group-include data=lib/extra_db/** --group-exclude docs=docs/archive/**
```

`core/src/storage/orders.rs` is in `data`. `core/src/storage/orders_test.rs` is
in `tests` only: it is excluded from `data`. `Cargo.toml` is in no group, so it
is in `ungrouped`.

## What is implemented

`core/src/groups/`:

- `config.rs`: `GroupConfig`, `GroupDefinition`, TOML parsing.
- `flags.rs`: `GroupFlags`, `load_group_config` (default file, explicit file,
  flags merged on top).
- `pattern.rs`: glob and `regex:` patterns.
- `matcher.rs`: `GroupMatcher::compile` (validates names, patterns, groups
  without patterns), `list_groups` (ends with `ungrouped`),
  `list_group_ids_for_file`.
- `error.rs`: `GroupConfigError`.

`core/src/analysis/group_changes.rs` computes the Groups view rows
(`GroupRow`, `GroupLine`, `GroupLink`, `GroupStats`) from changed files.

The `analyze_file_changes` request takes `group_flags`. The response adds:

- `groups`: only groups holding changed files, in config order, ending with
  `ungrouped`. Never empty when files changed.
- `group_rows`: one row per group in `groups`.
- `all_changes_row`: lists every group in `groups` (id `@all`, label "All
  changes by group"). It is the summary view.
- `files[].group_ids` and `callees[].file_path`.

Axum and Tauri pass these through; a bad group config is a 400 in Axum. The
review page (`/review/html`) always renders `GroupedReview` when files
changed. The form has a "Groups" section
(config file path, add/leave-out lines).

## Code footer in the functions view

In the functions view each ledger has a **Code** footer: for every changed
function in the file, the whole function with changed lines marked (green
added, rose removed, two line number gutters), like the legacy Elixir
`function_diff`. It is below the Outputs and Inputs columns, hides when the
ledger is collapsed, and each function block can be collapsed.

- `core/src/analysis/function_diff.rs`: `diff_function_sources` (line diff via
  `similar`) and `get_function_diff`, from the function source in the base and
  in the head. Added functions are all added lines, removed all removed.
- `AnalysisService::get_function_diffs` (`GetFunctionDiffsRequest`: repo path,
  base and head commit, one target per function with an old and a new id).
  Axum `POST /api/get_function_diffs`, Tauri command `get_function_diffs`,
  `BackendAdapter.getFunctionDiffs`.
- Frontend: `CodeFooter` loads one request per file when the ledger shows, via
  a `FunctionDiffLoaderContext` that `ReviewScreen` provides. Without the
  provider (design explorations) no footer is drawn.

## Files of any type

Every file changed between the base and the head is listed, not only parsed
Elixir files. `FileChange.kind` is `parsed` (functions, callers, calls) or
`unparsed` (any other file, or an Elixir file with no changed functions). An
unparsed file has no functions, callers or calls, `module` is its capitalized
file name (`lib/hello/world.md` is "World.md"), and `previous_path` is set when
it was renamed. It is in groups by path and counts in every `3 files` total.

- `git::list_changed_paths` (`git diff --name-status -z -M`, plus untracked
  files for the working tree) and `file_changes::add_unparsed_files`.
- `file_diff.rs`: `get_file_diff` compares the text in the base and the head,
  keeping 3 lines of context around each change (`⋯` marks a gap in the UI).
  Binary files return `is_binary`. Files past 2000 lines report
  `omitted_line_count`.
- `AnalysisService::get_file_diffs`, Axum `POST /api/get_file_diffs`, Tauri
  `get_file_diffs`, `BackendAdapter.getFileDiffs`.
- UI: the ledger shows only the header and a Code footer (closed, a click
  opens it). The node view shows only the header.

## Open all and Close all

Two buttons in the review toolbar. "Open all" expands every ledger (file and
group) and opens every code block. "Close all" collapses every ledger to its
header and closes every code block. A click on one ledger or block still
toggles it afterwards. `explorations/open_all.tsx` holds the command context
and `useOpenAllState`, which each ledger and code block uses for its state.

## Code footer layouts

The "Ledger, one code block" renderer is the default: one container for the
code of every function, with no per-function headers. Hovering a function name
shows only that function's code. "Ledger" keeps one closed block per function.

## Open questions for the pivot

- Should patterns without a `/` match at any depth, like `.gitignore`? (Not
  yet; decided to keep full-path matching.)
- Should the default group be hideable or renameable?
