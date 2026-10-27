# Decisions: rewriting the prototypes in the Starter Pack

The code reviewer prototypes in `../existing/` were rebuilt here on the
Starter Pack template, as an MVP of two features: **Reviews** (what a change
did, from the prototypes) and **Sessions** (Claude Code sessions in tmux, from
the Starter Pack). This records each decision made along the way, the options
considered, and what was left out on purpose.

## 1. Where the code lives

**Decision:** `new/` holds the rewritten app, `existing/` holds the prototypes
untouched, as laid out before the work started. `new/` is a Cargo workspace of
two crates: `core/` (the analysis library, moved over from the prototypes
without changes to its logic) and `tauri/src-tauri/` (the desktop app). The
Starter Pack's `tauri/` directory is the base of `new/tauri/`.

Options: put the app at the repository root straight away, or keep it under
`new/` next to `existing/`. Kept the user's layout. Moving `new/` to the root
and deleting `existing/` is one `git mv` once the prototypes are no longer
needed for reference.

The Starter Pack's `phoenix/` directory was not copied. Nothing in the MVP
needs it, and it carries build output and databases.

## 2. No web server for now

**Decision:** the Axum crate and the `BackendAdapter` layer are gone. The
Rust side is the `core` crate plus Tauri commands. The frontend calls the
commands through one module, `tauri/src/lib/backend.js`, and nothing else
calls `invoke`. When a web server comes back, that file is the seam: each
function keeps its name and its request and response shapes, and sends them
over HTTP or a websocket instead.

`core` stays a separate crate rather than being folded into the Tauri crate,
because it is already a clean library with its own tests and no Tauri
dependency, and splitting it out again later would cost more than keeping it.

## 3. Decoupling the Starter Pack from Phoenix

The Starter Pack's desktop app imports Svelte components Phoenix generates
(`$phoenix/components/*`), Phoenix's daisyUI theme and its theme script, and a
Rust crate from the Phoenix tree (`ecto_changeset`). **Decision:** copy the
four components the app uses (`Button`, `Header`, `Icon`, `ThemeToggle`), the
theme and the theme script into `tauri/src/lib/`, and drop everything else:
the websocket client, token store, offline sync, communities page, quick
search popup, the macOS bindings and their Cargo dependencies.

The theme keeps the Starter Pack's colours, daisyUI, the `phx:set-theme`
event and the `phx:theme` storage key, so when a Phoenix web UI joins the
app, the pieces line up again and the copies can go back to being imports.

## 4. Pages, one HTML entry each

**Decision:** keep the Starter Pack's one-HTML-file-per-page layout (Vite
multi-page), and name the pages by what they do:

| Page | File | Does |
| --- | --- | --- |
| Home | `src/index.html` | Links to the two features |
| New review | `src/reviews/new.html` | The form: repository, what to compare, rules, groups |
| Show review | `src/reviews/show.html?request=…` | Runs the comparison and shows it |
| Sessions | `src/sessions/index.html` | The tmux and Claude Code sessions |
| New session | `src/sessions/new.html` | Creates a session and opens it |

The review request travels from the form to the show page as JSON in one
query parameter. Options: a client-side router in one page, or server state.
Neither exists in the template; the query keeps every page independent and
makes a review reloadable and linkable. The form is also remembered in
`localStorage`, as in the prototype, so "Edit review" opens on the same
choices.

## 5. Naming the backend commands

**Decision:** the Tauri commands take the core crate's verbs where the core
already has a function, and list/get/create/show otherwise:

| Command | Core function |
| --- | --- |
| `list_branches_and_recent_commits` | `git::list_branches_and_recent_commits` |
| `analyze_file_changes` | `AnalysisService::analyze_file_changes` |
| `get_function_diffs` | `AnalysisService::get_function_diffs` |
| `get_file_diffs` | `AnalysisService::get_file_diffs` |
| `get_session_settings` | `sessions::detect_settings` (cached) |
| `list_sessions` | `sessions::list_sessions` |
| `create_session` | `sessions::create_session` |
| `show_session_in_terminal_app` | `sessions::show_session_in_terminal_app` |

`analyze_file_changes` was kept over something like `get_review` because
nothing is stored: "analyze" says what happens when it is called. The
template's `tmux_example` module became `sessions`, with `get_tmux_status`
renamed to `list_sessions` and `open_tmux_session` to
`show_session_in_terminal_app`, which says it focuses or opens a window.

Only the commands the two features use are exposed. The core crate's other
entry points (`analyze_changes`, `get_function_source`, `get_call_trace`,
`list_tests`, `get_test_call_stack`) stay in the library, tested, but have no
command, no page and no UI. The prototype's `greet` and `list_changed_files`
commands were removed.

## 6. The two review views

The prototype had many renderers (chips, blocks, ledger, several code
layouts, an "outside the project" toggle). **Decision:** exactly two:

- **Node view** (`src/reviews/nodes/`, SvelteFlow): the group and file level.
  One row per group on a canvas: the files outside the group that call in as
  chips on the left, the group card listing its files in the middle, the
  files it calls out to as chips on the right, with edges between them. The
  "All changes" row at the top lists the groups. Clicking a file opens the
  Ledger view of its group; clicking a group, or a chip of a file that is
  itself changed, scrolls the canvas to that group. Hovering a line fades the
  chips of the other lines.
- **Ledger view** (`src/reviews/ledger/`, plain HTML): the file and function
  level. The files of one group, each as a ledger with three columns (File:
  the changed functions; Outputs: calls into the project; Inputs: callers in
  other modules) and a Code footer. A tree on the left lists the groups and
  the open group's files.

Heights on the canvas are computed from the content (`nodes/layout.js`), as
in the prototype, so nothing measures the DOM.

The prototype drew the file-level chips view on ReactFlow too. It was left
out: the Node view covers groups and files, the Ledger view covers files and
functions, and one renderer per level keeps the code small.

## 7. SvelteFlow

`@xyflow/svelte` 1.7 replaces `@xyflow/react`. The custom nodes are Svelte
components that read what to highlight and what a click does from a Svelte
context the canvas sets, so they stay presentational, as the prototype's
nodes did with a React context. The canvas pans on scroll and zooms with
⌘ and the wheel, and follows the page's theme.

## 8. The code footer

**Decision:** one layout, the prototype's default: one block per file holding
the code of every changed function, closed at first. Hovering a function in
the ledger shows only its code; a click keeps it open. Files that were not
parsed (anything but Elixir with changed functions) show their changed lines
with context, and Markdown files can switch between the rendered HTML the
backend produces and the raw lines, as before.

Syntax colours use highlight.js on the frontend, as in the prototype.

## 9. Sessions: settings detected once, not edited

The template's tmux page had a settings form (shell, terminal app, tmux
path) the user could correct, passed with every call. **Decision:** the Rust
side detects the settings once per app run (`AppState.session_settings`, a
`tokio::sync::OnceCell`) and every command uses them. The sessions page shows
what was detected, read-only. This removed a form, the per-call settings
arguments and the state the two pages would otherwise have had to share. The
form is listed in `docs/suggestions/sessions.md` for when a wrong guess
actually bites.

The template's list and create were on one page; they are now
`sessions/index.html` and `sessions/new.html`, following index/new.

## 10. Sessions: only the app's own, by name prefix

The template listed every tmux session on the user's server, window by
window and pane by pane, with the processes and git status of each pane, and
every running Claude Code session underneath. For this app the question is
narrower: which review sessions did it start, and is Claude Code busy or
idle in each. **Decision:** every session the app creates is named
`<prefix>-<name>`, with the prefix `cr` (a constant in `sessions.rs`, held
in `Settings.session_name_prefix` so the pages read it and a settings form
could edit it), and the list shows only sessions named so. The new session
form shows the prefix in front of the name.

For each listed session the Rust side finds the Claude Code session running
in one of its panes: the one whose status file in `~/.claude/sessions` names
the pane, else a `claude` process on one of the panes' ttys, which has no
status (shown as "unknown"). The card shows that status, the directory of
the current pane, and the Copy and Open buttons. The pane rows, process
lists, git statuses and the separate Claude Code list were dropped (see
`docs/suggestions/sessions.md`), and with them the app's `git.rs` module.

Options considered: a tmux environment variable or a session option to mark
the app's sessions instead of the name. The name is visible in `tmux ls` and
in the terminal window title, costs one `list-sessions` call to filter, and
survives tmux restarts the same way; the others would need a second call per
session.

## 11. Sessions: a new session is a worktree

The template's form made a tmux session in any directory. A review session
here is meant to work on a branch of its own without touching the user's
checkout. **Decision:** the form takes a repository instead of a directory,
and `create_session` first adds a git worktree at
`<repository>/.worktrees/<name>` (`git_worktree.rs`), on a branch named
`<name>` created from the repository's current HEAD, or on the existing
branch of that name, then starts the tmux session in it. The name is one
value for the branch, the worktree directory and the tmux session
(`cr-<name>`), so the session name validation covers the branch name too.

`/.worktrees/` is appended to the repository's `.git/info/exclude` the first
time, when git doesn't already ignore it, so the worktrees never show as
untracked in the main checkout and nothing has to be committed. A
repository without commits is refused with a message, since there is no
HEAD to branch from. The worktree is left in place when the tmux session
ends; removing it is listed in `docs/suggestions/sessions.md`.

Options: Claude Code's own `.claude/worktrees/<name>` layout, or a sibling
directory next to the repository. `.worktrees/` keeps the app's worktrees
apart from the ones Claude Code manages (it cleans up its own), and inside
the repository where they are found again.

## 12. Left out on purpose

Each of these existed in a prototype or the template and was dropped from
the MVP. See `docs/suggestions/` for the list to bring back from.

- The Axum server, the shared React frontend and `BackendAdapter`.
- The explorations pages, the test call stack pages and the ReactFlow
  file-level views.
- "Open all" and "Close all" buttons; the "Dimmed / Hidden" toggle for calls
  into dependencies (they are always folded into a count now).
- The Phoenix window, login, websocket client, offline sync and quick search.
- Editable session settings.
- Opening links from rendered Markdown in the browser (clicking one does
  nothing).

## 13. Verification

`bin/check` runs what `AGENTS.md` asks for: `cargo fmt --check`, `cargo
clippy --workspace --all-targets -D warnings`, `cargo test --workspace`,
`cargo audit`, plus `pnpm check` (svelte-check) and `pnpm build`.

## 14. What was verified

On 2026-10-27, with `bin/check` passing (fmt, clippy, 126 Rust tests,
svelte-check, Vite build, cargo audit with two allowed unmaintained-crate
warnings from Tauri's dependency tree):

- `cargo tauri dev` builds and opens the app without errors.
- The pages were driven in headless Chrome against the Vite dev server, with
  Tauri's `invoke` bridged over HTTP to the same Rust code the commands call
  (a throwaway bridge outside the repository). Checked by screenshot: the
  home page; the new review form reading the branches of a repository and
  submitting; the show page in the Node view with group cards, chips and
  edges, hover fading, a chip click scrolling to a group, light and dark;
  the Ledger view with the tree, the three columns, markers, hovering a
  function to preview its code and pinning it, and a jump link scrolling to
  another file; the sessions page listing tmux and Claude Code sessions; the
  new session form with the detected directory.
- Later the same day, after decision 10: the sessions page listing only
  `cr-` sessions (one with a shell at a prompt, created with tmux directly
  for the check) and the new session form showing the `cr-` prefix, driven
  the same way.
- After decision 11: `git_worktree.rs` is covered by tests on temporary
  repositories (a new branch, an existing branch, a repository without
  commits, the exclude file). Creating a session end to end opens a terminal
  window, so that was left to the user.
- Not exercised: creating a session and opening one in the terminal app
  (both would open windows on the Mac the tests ran on). The code paths are
  the Starter Pack's, unchanged apart from where the settings come from.
