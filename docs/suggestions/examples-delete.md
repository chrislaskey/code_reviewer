# Deleting the examples: what is still hard

Found 2026-11-25 deleting posts and files from Code Reviewer by the steps
in `docs/design/examples-delete.md`. The framework no longer names them,
and the app built and passed with no `[[divergence]]`. These are what
still took work, or still could break an app.

## Keep the example's globs in framework.toml

The design's step 1 says to delete the example's globs from `examples`
after its files. But `bin/framework update` reads those globs: when the
template changes an example the app deleted, `framework_check classify`
names the file an example, and the deletion is kept. With the glob gone,
the file is the app's, and the merge stops on a conflict. Code Reviewer
kept its globs.

- **Suggested**: say in the design to keep the globs. The check that no
  framework file names an example only gets stricter with them.

## The sync tests over HTTP sit in a seam

`axum/src/tests/app.rs` is a seam, but it holds framework tests on posts:
`sync_push_applies_an_event_and_pull_returns_the_row` and
`sync_push_takes_a_request_up_to_the_most_a_device_sends_and_no_larger`.
An app that deletes posts must rewrite them or lose them. Code Reviewer
moved them onto `app_test_notes`. The bearer token test in
`axum/src/tests/router.rs` uses posts too, and could use
`/api/users/current`.

- **Suggested**: move them to a framework test file, such as
  `axum/src/tests/sync.rs`, on test notes.

## The files example's dependencies are in tauri/Cargo.toml

The macOS dependencies of the clipboard and the quick picker
(`tauri-plugin-global-shortcut`, `tauri-nspanel`, `core-graphics`,
`core-foundation`, the `objc2` AppKit features) stay after files is
deleted. Nothing fails, since an unused crate gives no warning, but the
app still builds them.

- **Suggested**: a comment in `tauri/Cargo.toml` saying they are the
  files example's, and listing the step in the design. The iOS clipboard
  features of `objc2-ui-kit` are the same.

## An app with a deployed database adds its own drop migrations

The design says so. Code Reviewer had nothing to keep, so it deleted the
migrations and reset instead, by `bin/reset`. That showed a bug, fixed:
`bin/reset` deleted `com.starterpack.app`'s files whatever the app, and
now reads the identifier from `tauri/tauri.conf.json`. A phone's app is
not reset by it; reinstalling the app is.

- **Suggested**: give the two drop migrations in the design, ready to
  copy, for an app that keeps its database. Each deletes its tables'
  rows from `app_events`, then drops the tables, children first.

## Deleting an example edits many seams by hand

Each resource is a few lines in many shared lists: modules, synced
tables, routes, error conversions, Tauri commands, page paths, URLs and
core calls. Deleting posts and files edited 15 seam files. Most of it
could become deleting files:

- **The pages' calls per resource**: `svelte/src/lib/core/http-api.js`
  and `tauri-commands.js` split by resource, as
  `svelte/src/lib/core/posts/http-api.js` and `tauri-commands.js`, with
  `index.js` one line per resource.
- **Routes per resource**: each module under `axum/src/app/` with its own
  `routes()`, merged by `router.rs` in one line.
- **Errors per resource**: each `From<PostError> for ApiError` in the
  resource's axum module, not in `api_error.rs`. The same for
  `CommandError` in `tauri/src/command_error.rs`.
- **Commands per resource**: harder, since `tauri::generate_handler!` takes
  one list. A macro per module that lists its commands could be spliced in.
- **Pages per folder**: each page folder exporting its paths, URLs and
  page imports, with `routes.js` and `pages.js` one line per folder.

Each is a change to how every app adds a feature, so each needs a plan
and a changelog entry of its own.
