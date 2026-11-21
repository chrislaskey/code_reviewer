# Framework changes

What an app built from this template does about a template change that
asks something of it, newest first. The template's version is its
commit; there are no releases. A change that needs nothing from an app
is not listed: `bin/framework update` merges it. See
`docs/design/template-and-apps.md`.

Each entry: the date and the commit, what changed, what an app does.

## 2026-11-21: the template's own files, checked

- **What changed**: sync runs over a registry of synced tables
  (`core/src/app/synced_tables.rs`) and moved to `core/src/sync/`; the
  bytes of synced rows are `core/src/sync/bytes.rs`; jobs are
  `core/src/jobs.rs` with an app's own in `core/src/app/jobs.rs`;
  `framework.toml` and `framework.lock` name and check the template's
  files.
- **What an app does**: list each synced table of its own in
  `synced_tables.rs`, implementing `SyncedTable` (and `SyncedBytes` for
  one with bytes) in the table's module, in place of the `match`
  statements sync used to have; add its jobs to `core/src/app/jobs.rs`;
  move its own changes to template files into `[[divergence]]` entries
  in `framework.toml`, or send them back with `bin/framework contribute`.
