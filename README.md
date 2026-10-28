# Code Reviewer

A macOS desktop app with three features:

* **Reviews**: what a change did in an Elixir or Rust repository, function by
  function, file by file and group by group. Pick a repository and what to
  compare (uncommitted changes, one commit, or a branch against another), then
  read it on a canvas of groups and files, or as ledgers of functions, calls,
  callers and code.
* **Traces**: a test of the same comparison, with every call below it walked
  top to bottom like a stack trace. Changed functions show their diff,
  unchanged ones their name, calls outside the project a slim row, to read
  what a change does on the path a test takes.
* **Sessions**: the tmux sessions this app started, named `cr-<name>`, with
  the Claude Code status (busy, idle) in each, and a form to start a new one:
  a git worktree of a repository and a tmux session, both named `cr-<name>`,
  running Claude Code, opened in your terminal app. After a restart the
  worktrees are found again in the repositories under `~/code`, and a
  session can be continued in one. Each shows its git status, and is
  finished from its Complete menu: Merge into `main` (fast-forward `main`,
  remove the worktree, delete the branch) or Turn into branch (remove the
  worktree, keep the branch), as `bin/worktree` does. Its Review and Trace menus open
  the worktree's uncommitted changes, or everything its branch committed
  against `main`, in Reviews or Traces.

It is built on the [Starter Pack](../existing/) template's Tauri app, with
the analysis from the earlier prototypes in `../existing/`. The decisions
behind the rewrite are in `docs/decisions.md`.

## Layout

```
core/            the analysis library: git comparisons, function changes, call links, groups
tauri/           the desktop app
  src/           the pages, a Svelte app with one HTML entry per page (see tauri/README.md)
  src-tauri/     the Tauri crate: the commands the pages call
docs/            decisions.md, and suggestions/ for what was left out
bin/check        every check: fmt, clippy, tests, svelte-check, build, audit
bin/dev          cargo tauri dev
```

## Running

Prerequisites: Rust (stable), the Tauri CLI
(`cargo install tauri-cli --version '^2' --locked`), Xcode command line
tools, Node and pnpm (see `tauri/.tool-versions`), and tmux for the sessions
pages.

```sh
bin/dev
```

`bin/check` runs the checks `AGENTS.md` asks for before a task is done.

## Adding a feature

1. Add the function and its request and response types to `core/`.
2. Add a Tauri command in `tauri/src-tauri/src/` that calls it, list it in
   `build.rs` and allow it in `capabilities/default.json`.
3. Add a function for it to `tauri/src/lib/backend.js`.
4. Use it from a page under `tauri/src/`.
