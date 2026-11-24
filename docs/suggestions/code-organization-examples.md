# Examples an app can delete

`docs/design/template-and-apps.md` says posts and files are examples, an
app's to replace or delete. Found 2026-11-23 moving Code Reviewer onto
the template: an app can take them out of the seams (routes, commands,
`synced_tables.rs`, nav, pages), but not delete them, since the
framework still names them:

- **Code.** Deleting an account deletes its posts and files by name
  (`accounts/deletions.rs`); `communities.rs` uses `files_communities`
  and `memberships.rs` uses `posts_communities`; `trial_users.rs` and
  `local_users.rs` refer to them too.
- **Tests.** About 150 references in framework tests use posts and
  files as the synced table to test with, 110 of them in `sync.rs`,
  more in `purge.rs`, `pending_changes.rs`, `accounts.rs`,
  `device_accounts.rs` and `axum/src/tests.rs`.

Suggested:

- What the framework does to every table, an account's deletion and a
  community's or membership's rows, through `SyncedTable` hooks, as
  pruning and backfill are.
- The framework's tests on a table of their own, compiled for tests
  alone, so they keep running when an app deletes the examples.
- A test that deletes the examples' files and still builds, so the
  promise stays kept.
