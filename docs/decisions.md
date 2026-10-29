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
| Show review | `src/reviews/show.html?repo_path=…` | Runs the comparison and shows it |
| New trace | `src/traces/new.html` | The form: repository, what to compare (decision 20) |
| Show trace | `src/traces/show.html?repo_path=…&test_name=…` | Lists the tests and walks the calls below the picked one |
| Sessions | `src/sessions/index.html` | The tmux and Claude Code sessions |
| New session | `src/sessions/new.html` | Creates a session and opens it |

The review request travels from the form to the show page in the query
(at first as JSON in one parameter, since decision 14 as named parameters).
Options: a client-side router in one page, or server state. Neither exists
in the template; the query keeps every page independent and makes a review
reloadable and linkable. The form is also remembered in `localStorage`, as
in the prototype, so a new review starts from the last one.

## 5. Naming the backend commands

**Decision:** the Tauri commands take the core crate's verbs where the core
already has a function, and list/get/create/show otherwise:

| Command | Core function |
| --- | --- |
| `list_branches_and_recent_commits` | `git::list_branches_and_recent_commits` |
| `analyze_file_changes` | `AnalysisService::analyze_file_changes` |
| `get_function_diffs` | `AnalysisService::get_function_diffs` |
| `get_file_diffs` | `AnalysisService::get_file_diffs` |
| `list_tests` | `AnalysisService::list_tests` (decision 20) |
| `get_test_call_stack` | `AnalysisService::get_test_call_stack` (decision 20) |
| `get_session_settings` | `sessions::detect_settings` (cached) |
| `list_sessions` | `sessions::list_sessions` |
| `create_session` | `sessions::create_session` |
| `show_session_in_terminal_app` | `sessions::show_session_in_terminal_app` |

`analyze_file_changes` was kept over something like `get_review` because
nothing is stored: "analyze" says what happens when it is called. The
template's `tmux_example` module became `sessions`, with `get_tmux_status`
renamed to `list_sessions` and `open_tmux_session` to
`show_session_in_terminal_app`, which says it focuses or opens a window.

Only the commands the features use are exposed. The core crate's other
entry points (`analyze_changes`, `get_function_source`, `get_call_trace`)
stay in the library, tested, but have no command, no page and no UI
(`list_tests` and `get_test_call_stack` got theirs in decision 20). The
prototype's `greet` and `list_changed_files` commands were removed.

## 6. The two review views

The prototype had many renderers (chips, blocks, ledger, several code
layouts, an "outside the project" toggle). **Decision:** exactly two:

- **Node view** (`src/reviews/nodes/`, SvelteFlow): the group and file level.
  One row per group on a canvas: the files outside the group that call in as
  chips on the left, the group card listing its files in the middle, the
  files it calls out to as chips on the right, with edges between them. The
  "All changes" row at the top lists the groups. Clicking a file opens the
  Ledger view of its group; clicking a group, or a chip of a file that is
  itself changed, moves the canvas to that group. Hovering a line fades the
  chips of the other lines.
- **Ledger view** (`src/reviews/ledger/`, plain HTML): the file and function
  level. The files of one group, each as a ledger with three columns (File:
  the changed functions; Outputs: the calls they make, with calls into
  dependencies and the standard library dimmed or hidden by a toolbar
  toggle; Inputs: callers in other modules) and a Code footer. "Open all"
  and "Close all" on the toolbar open or close every ledger and code block.
- **The tree** (`src/reviews/ReviewTree.svelte`) sits beside both views, like
  the file list of a pull request: All changes, every group under it, and
  the files of the chosen group under that group. One click always means the
  same view, as in the prototype: All changes and a group show the Node view
  moved to that row (a group also lists its files); a file shows the Ledger
  view of its group scrolled to the file.

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
parsed (anything but Elixir or Rust with changed functions) show their changed lines
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
`<repository>/.worktrees/cr-<name>` (`git_worktree.rs`), on a branch named
`cr-<name>` created from the repository's current HEAD, or on the existing
branch of that name, then starts the tmux session in it. The branch, the
worktree directory and the tmux session share the one name, prefix
included, so the worktree is matched to its session by name alone and the
session name validation covers the branch name too.

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

## 12. Sessions: worktrees outlive tmux and can be continued

After a restart the tmux sessions are gone but the worktrees remain, with
the work and Claude Code's conversation in them. **Decision:** the list
also shows the app's worktrees with no session running, under the sessions,
with a Continue button that starts a tmux session named after the worktree
in it, types `claude --continue` (a constant, `CONTINUE_COMMAND`) and opens
it in the terminal app.

The worktrees are found in the repositories under watched directories: a
configurable list (`Settings.watched_directories`, `~/code` by default),
each looked at together with its first-level subdirectories, and `git
worktree list --porcelain` run in every one that has a `.git` entry. Only
worktrees whose directory is named `cr-…` count, as with the sessions. A
worktree whose session is running shows as the session, not twice.

Options: record the worktrees the app creates in a file of its own, or scan
for worktrees with `find`. Asking git in the watched repositories needs no
state the user could lose or that could go stale, and the first-level scan
costs one `git worktree list` per repository every refresh, which is quick
for a directory of a few dozen repositories.

## 13. Sessions: finished from the list, with the git status in view

`bin/worktree` (on `main` since the rewrite) finishes a worktree from inside
it: `merge` fast-forwards `main` with the branch, removes the worktree,
deletes the branch and closes the tmux session; `close` removes the worktree,
keeps the branch and closes the session. Both refuse uncommitted changes.
**Decision:** the same two actions on every session and worktree card,
done by the Rust side (`git_worktree::
merge_worktree` and `close_worktree`, then `tmux kill-session`) with the
same refusals and the same order, git first and tmux last so a refusal
leaves the session running. The main branch is a setting
(`Settings.main_branch`, `main`) like the prefix. The command answers with a
sentence saying what was done, which the page logs.

Each card also shows the git status of its directory, as a shell prompt
does: the branch, coloured by whether the worktree is dirty, then counts of
staged, unstaged, untracked and conflicted files and how far the branch is
ahead of or behind its upstream. This brought back the `git.rs` module and
`GitStatus.svelte` dropped in decision 10, now once per session or worktree
rather than once per pane.

The two actions sit behind one Complete button with a caret, a dropdown of
"Merge into main" and "Turn into branch", each with a sentence saying what
it does, since the script's `merge` and `close` names don't say what
happens to the branch. Both choices are disabled while the status is dirty,
with the reason under them, since the Rust side would refuse.

No confirmation dialog: as with the script, nothing is lost. Merge into main
is a fast-forward and Turn into branch keeps the branch, and both refuse a
dirty worktree.

## 14. Pages link to each other by URL, through one module

The Sessions list wanted a way into Reviews: review the uncommitted changes
of a worktree, or everything its branch changed against `main`. The first
such link between the two features, and more will follow (a session from a
review, a commit from a card).
**Decision:** a page's input is its query, flat and named the way a web
server would read it (`/reviews/show.html?repo_path=…&base_ref=main&
head_ref=cr-fix&compare_from_merge_base=true`, a list as a repeated key),
and `src/lib/routes.js` is the one module that knows every page's path and
parameters: `reviewShowUrl(request)`, `reviewNewUrl(request)`,
`sessionsIndexUrl()` and so on, and `reviewRequestFromSearch(search)` for
the page reading its query. The parameter names are the core crate's request
fields, so the URL is the request. Every link in the app goes through it,
the nav strip and the home page included, and nothing else spells out a
path. The show page's `request` JSON parameter went away: a named query is
readable, hand-writable and what a web server would take unchanged, so the
same links keep working if a server joins the app. "Edit review" now links
to the new page with the request in the query, and the form starts from it
(`formFromRequest`) rather than from `localStorage`.

So the Review menu on a card is a `ChoiceMenu` of plain links: "Recent
changes" is `reviewShowUrl({ repo_path })`, "All changes" is the branch
against the main branch from the merge base. Nothing in the sessions code
knows how reviews work, and nothing in the reviews code knows about
sessions. The menu itself became shared with Complete: `ChoiceMenu` (the
button, the caret, closing) and `MenuChoice` (a label, a sentence, a link
or a button) in `src/lib/components`.

## 15. Rust files, read the same way as Elixir

Reviews of this app itself need its Rust read as Elixir is: functions by
file, calls out, callers in, tests. **Decision:** a second `LanguageAnalyzer`
on the tree-sitter-rust grammar (`core/src/analysis/rust.rs`), with the
shared index, changes, diffs and call links untouched apart from two
additions. Everything after parsing is still language independent.

What a Rust function is called: the file's module path from the Cargo
layout (`core/src/git.rs` is `core::git`, `src/lib.rs` is `crate`, a
nested `mod tests` is `core::git::tests`), plus the type for methods
(`core::git::Repo.open/1`). The arity leaves out `self`, so it is the number
of arguments a call writes. A one-function `impl` block (`impl From<A> for
B`, `impl Display for B`) is read as that function with its header; a type
with several such impls merges them into one function with several clause
ranges, as Elixir clauses are. `#[test]` functions are tests. Doc comments
and attributes directly above a function belong to it.

How calls are matched, without a type checker:

- Paths follow `use` declarations, `crate`, `super`, `self`, `Self` and the
  file's own items: `Billing::charge(x)` and `self.total()` name one module.
  `use super::*` in a test module makes an unknown `Foo` any `Foo` of the
  globbed modules. These resolve only against functions the project
  defines (`RawCallTarget::ByName`, new), so a derived or standard method
  reached through a project type (`Foo::default()`) is dropped, not shown
  as an outside call.
- A method call on any other value (`order.total()`) is matched by name
  and arity alone, and only when the project defines exactly one such
  function. Names the standard library uses everywhere (`get`, `push`,
  `len`, `iter`, ...) are never matched this way.
- Paths outside the crate (`std::fs::read_to_string`, `serde_json::from_str`)
  are calls outside the project, dimmed or hidden as Elixir dependencies
  are. A path into another workspace crate uses the package name
  (`code_reviewer_core::git`) while the index names the crate by its
  directory (`core::git`); the resolver accepts a module that ends the same
  way after the first segment when there is exactly one (the second shared
  addition).
- Arguments of macros (`assert_eq!`, `format!`, `vec!`) are parsed again as
  expressions, in place, so calls inside them are found with their lines.
- `Self::helper` or `Type::helper` passed as a value (to `map`) is a call
  with unknown arity. A bare `helper` passed as a value is not: it cannot
  be told from a variable.

Module rules accept `::` paths as well as `.` ones: `core::git` covers
`core::git::Repo`, and `core::git::run_git` or `core::git.run_git` names
the function. Function labels keep the tool's `Module.name/arity` form for
both languages. The ledger highlights function code in the file's language
instead of always Elixir.

## 16. Sessions: a port per session, in `PORT`

Most repositories reviewed here are web apps with a dev server, and two
worktrees of the same repository start it on the same port, so the second
fails or, worse, the user tests the wrong one. **Decision:** every session
gets a port of its own, set in its tmux environment as `PORT` (`tmux
new-session -e`, tmux 3.2 or later), so the shell and everything started in
the session see it. The new session form has a Port field, filled in with
the lowest port from 4001 up that none of the app's running sessions holds
(`get_default_session_port`, read back with `tmux show-environment`), which
with no gaps is 4000 + the session count + 1; the user may type another,
and a port another session holds or one below 1024 is refused. Continue
takes the default without asking. The session card shows the port.

The repository decides what to do with the number. This repository's
`bin/dev` makes Vite listen on `PORT` and points Tauri at it with
`--config`, keeping 1420 when `PORT` is unset so nothing changes outside a
session. A repository with several servers (a Phoenix app with an Axum
server, say) spreads them from `PORT`, such as `PORT` for the first and
`PORT + 1000` for the next, so consecutive session ports never meet. The
same number can tell dev databases apart (`myapp_dev_4001`).

Options considered: a small slot number (1, 2, 3) with each repository
deriving `4000 + slot * 10 + offset`, which handles several servers with
blocks of ten but makes every repository do arithmetic before a
single-server app works, and leaves the form showing a number nobody can
paste into a browser; named ports the repository declares in a file of its
own, which is the most explicit but adds a format to maintain; and dynamic
ports each server picks at start, which need no allocation but change
between runs. `PORT` works with no change for the many servers that already
read it, and the form shows the address the app will be on.

The port is not remembered with the worktree: a continued session gets the
lowest free port again, which can differ from the last run. Remembering it
is in `docs/suggestions/sessions.md`.

## 17. Left out on purpose

Each of these existed in a prototype or the template and was dropped from
the MVP. See `docs/suggestions/` for the list to bring back from.

- The Axum server, the shared React frontend and `BackendAdapter`.
- The explorations pages and the ReactFlow file-level views. The test call
  stack page came back as Traces in decision 20; its canvas version did not.
- The "Folded" choice for calls into dependencies (a "+N in dependencies and
  stdlib" line); only "Dimmed / Hidden" remain.
- Highlighting the tree item of the section on screen as the page scrolls.
- The Phoenix window, login, websocket client, offline sync and quick search.
- Editable session settings.
- Opening links from rendered Markdown in the browser (clicking one does
  nothing).

## 18. Verification

`bin/check` runs what `AGENTS.md` asks for: `cargo fmt --check`, `cargo
clippy --workspace --all-targets -D warnings`, `cargo test --workspace`,
`cargo audit`, plus `pnpm check` (svelte-check) and `pnpm build`.

## 19. What was verified

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
- After decisions 11 and 12: `git_worktree.rs` is covered by tests on
  temporary repositories (a new branch, an existing branch, a repository
  without commits, the exclude file, listing the worktrees under a watched
  directory). The sessions page was driven in headless Chrome again with a
  `cr-` worktree of a temporary repository under a watched directory,
  showing the worktree card with Continue. Creating or continuing a session
  end to end opens a terminal window, so that was left to the user.
- After decision 13: merging and closing are covered by tests on temporary
  repositories (a merge, a dirty worktree refused, a diverged branch
  refused, a clean close keeping the branch). The page was driven again
  with a temporary repository holding a clean and a dirty worktree and a
  tmux session in the clean one: the git status showed on each card, the
  choices were disabled on the dirty one, and Merge into main on the clean one
  merged, removed the worktree and closed the session through the page.
  The Complete menu was then checked the same way with a canned `invoke`
  (no Rust side), as only the page changed: both choices, the sentences,
  disabled on a dirty worktree, closing on an outside click and on Escape.
- After decision 14: the Review menu's links were followed in the same
  headless setup: the show page sent the request parsed from the flat query
  to `analyze_file_changes`, Edit review opened the form filled in from it,
  and a flat link (with repeated `group_include` parameters) against the
  real core on a test repository showed its groups and files.
- After decision 15: `bin/check` passing (154 Rust tests, the new
  ones in `core/src/analysis/rust.rs` and `core/tests/rust_analysis.rs`).
  Indexing this repository found 562 functions in 43 Rust files, 154 of
  them tests, with 968 calls resolved to project functions and 247 outside
  it, including a Tauri command's call into `core` through the package
  name. The show page was driven in headless Chrome on the worktree's own
  uncommitted changes with `invoke` bridged to the real core: the Rust files
  were parsed, the ledger listed `name/arity` functions with their outputs
  and callers across files, and the code footer showed doc comments,
  attributes and Rust syntax colours.
- Not exercised: creating a session and opening one in the terminal app
  (both would open windows on the Mac the tests ran on). The code paths are
  the Starter Pack's, unchanged apart from where the settings come from.
- After decision 16: `tmux.rs` creates a session with a variable set and
  reads it back on a private tmux server, the lowest free port and the
  port check are unit tests, and `bin/check` passes. Vite started with
  `PORT=1422` listened there. The new session form and the list were driven
  in headless Chrome with a canned `invoke`: the Port field filled in from
  `get_default_session_port`, the form sent the typed port as a number, and
  the cards showed `port 4001` and nothing for a session without one. Not
  exercised: `bin/dev` opening the Tauri window on the overridden dev URL,
  and a session created end to end with `PORT` in its environment (both
  open windows).
- After decision 20: `bin/check` passing. The trace pages were driven in
  headless Chrome with `invoke` bridged to the real core: the new form
  submitting to the show page; this repository's 154 Rust tests listed and
  one walked (101 frames, 152 with system calls shown); a two-commit Elixir
  repository where the walked test reached a modified function, shown as a
  card with its diff, with Prev and Next change stepping to it and Close
  children and Close code folding the stack; the picked test kept in the
  URL across a reload and Edit trace filling the form. The sessions page
  was driven with a canned `invoke`: both cards show Review and Trace menus
  with the renamed choices linking to the two show pages.

## 20. Traces: the test call stack, back as a feature

The prototype's spike listed the tests of a comparison and walked the calls
below one, top to bottom like a stack trace, with each project function
shown in full and marked against the base (`core/src/analysis/
test_call_stack.rs`, kept in the library with its tests since decision 5).
**Decision:** bring it back as the third feature, **Traces**, built the
same way as Reviews: a new page with the form (`src/traces/new.html`), a
show page that takes the comparison from its query (`src/traces/show.html`),
two Tauri commands named after the core functions (`list_tests`,
`get_test_call_stack`, in `trace.rs`), two functions in `backend.js`, and
`traceNewUrl`, `traceShowUrl` and `traceRequestFromSearch` in `routes.js`.
The Trace link sits next to Review in the nav, and every session and
worktree card has a Trace menu next to its Review menu with the same two
choices.

The show page lists the tests in the head on the left (by file, with a
search box, and a "LiveView tests only" switch when the repository has
any), and the picked test's stack on the right: changed functions as cards
with their diff, unchanged project functions as one-line cards, calls
outside the project as slim rows, with guide lines for the depth. The
toolbar shows or hides the system calls and the unchanged functions, opens
or closes every frame's code or children, and steps between the changed
frames. A click on a frame closes its code and the frames below it, or
opens both again. The picked test goes into the URL (`test_module`,
`test_name`, `test_arity`, the `FunctionId` fields) with
`history.replaceState`, so a reload or a bookmark lands on the same stack,
and "Edit trace" opens the form with the comparison filled in. The plain
HTML renderer was kept and the ReactFlow one left out
(`docs/suggestions/traces.md`).

Rather than copy the review pages, what both features need moved to
`src/lib/`, where the next feature finds it too:

- `comparison_form.js`: the comparison fields of a form (repository, mode,
  commit, branches), `buildComparison`, `comparisonFormFromRequest`,
  `describeComparison`, `describeRequest`, and `loadForm`/`saveForm` with
  a storage key per page. `reviews/review_form.js` builds on it, adding the
  module rules and groups.
- `components/ComparisonFields.svelte`: those fields as markup, reading the
  repository's branches and commits as the review form did.
- `components/SegmentedControl.svelte`, `components/Toolbar.svelte`,
  `components/CodeLines.svelte`, `syntax_highlight.js` and
  `change_styles.js`, moved out of `src/reviews/`.
- `code_page.css`: the layout of a page that fills the window
  (`.full-page`, was `.review-page`), the dotted background and the syntax
  colours, imported by both features' `app.css`.
- `routes.js` reads and writes the comparison parameters in one place for
  both request shapes.

On the Rust side the `run_analysis` helper (the blocking thread around the
core) moved from `review.rs` to `analysis.rs`, used by both command modules.

The sessions cards' two menus are one component, `ChangesMenu` (was
`ReviewMenu`), given the label, the title and the page function to link to
(`reviewShowUrl` or `traceShowUrl`). Its choices were renamed on the way:
"Recent changes" is now "Uncommitted changes" and "All changes" is
"Committed changes", since the old names did not say which is which.

Options considered: one show page with a view switch (Review / Trace)
instead of a feature of its own. The two take different inputs (a review
has rules and groups, a trace has a test), show different things and are
linked to separately from a session card, so separate pages keep each
page's query the request, as decision 14 wants.

## 21. Layout: a navigation gutter instead of a nav pill

The pages read as a website in a webview: a rounded nav pill across the
top, content centred under it. **Decision:** the navigation is a gutter
down the window's left edge (`AppNav.svelte`), with one square-ish tab per
page, an icon over one word: Home, Sessions, Review, Trace. The open tab is
joined to the page like a file folder's tab, and the page sits on top of the
gutter: the gutter is half a shade darker than the page, and the page's
edge with the open tab is one shape carrying the raised surfaces' ring and
shadow. The theme menu is a tile at the bottom of the gutter, whose list
opens up and to the right. Every page keeps its header (Sessions, New
review, …), set against the gutter rather than centred, and the two pages
that fill the window got a title row (Review, Trace) of the same size and
at the same top and left margins, with the comparison beside the title,
where the info bar was. The waves and the flow lines stay, starting where
the page does; the flow lines fade in from the left so the open tab meets
them softly.

The app name is gone from the chrome, and so is the title bar: the window
uses macOS's overlay title bar (`titleBarStyle: "Overlay"`, `hiddenTitle`
in `tauri.conf.json`), so the page and the gutter run to the top of the
window and the traffic lights sit in the gutter, as in Finder's or Mail's
sidebar. The native bar had been the OS's white or grey above a gutter
half a shade darker, with no edge between them, and it followed the OS
appearance rather than the app's theme. The gutter leaves 44px above its
first tab for the lights (`trafficLightPosition` puts them at 16, 16), and
the pages keep the bar's height clear above their titles (`--page-top`).
Since nothing native remains to drag the window by, the bar's strip over
the page, the room around the lights and the empty part of the gutter are
`data-tauri-drag-region`, which needs `core:window:allow-start-dragging`
in the capabilities; double-clicking them zooms the window as the title
bar did. Considered instead: ringing and shadowing the gutter's top edge
under the native bar, one CSS rule that reads the bar as part of the raised
page surface, which stays the fallback if the overlay misbehaves. The Home
tab may go later, once the home page has a purpose beyond linking to the
other three.

How the shape is drawn: the page's edge, top to bottom, bulging left around
the open tab with concave curves where they meet, is an SVG path
(`src/lib/folder_tab.js`) computed from the open tile's position and
redrawn when the gutter resizes, with a `drop-shadow` filter so one shadow
follows the whole outline, curves included. The shape is clipped at the
page's edge, so the 2px overlap that hides any seam never covers the page,
while the theme list can still open out of the gutter. Pure CSS was tried
first: a straight junction works with box shadows, and a curved one with
the inverted-corner trick, but the curve then carries no ring or shadow.

The gutter is `position: fixed`, like the waves and the pattern, and the
body leaves its width free with `padding-left`, so landing pages scroll as
before. Picked from static mockups served by the dev server for the
purpose and removed once decided (fourteen wide concepts, then icon rail,
inset sheet, unified toolbar, folder tab and title bar deep dives): the
user ruled out the inset sheet and an app mark in the rail, and chose the
curved tabs from the window edge over a straight junction and a
square-edged band. The tree beside the Review and Trace content shares the
page's background rather than a darker one, so the gutter is the only
darker surface in the window.
