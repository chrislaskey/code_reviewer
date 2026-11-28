# Template and apps

Status: built 2026-11-21, steps 1 to 9.

## Request

This repository is a template. Each app starts as a copy and replaces
what it needs: tables, pages, features. That is expected, and an app may
diverge. What should stay possible:

- **Pulling down**: an app keeps taking the template's improvements
  without a painful merge each time.
- **Contributing back**: a refinement found while building an app goes
  back into the template, then down to the other apps.

No release cycle, semver or versioned docs: too much overhead for this
project. Instead, light guardrails that tell a person (or an agent) when
they are changing the template's code, so they choose consciously:
extend it from the app's own files, change it upstream and pull it down,
or diverge on purpose and say why. A warning or a failed check, like a
linter, never a block.

## Why

Decided 2026-11-18: an app is a copy of the template with the template
as a second git remote, `template`. Updates arrive by merging
`template/main`. Generic changes go in their own commits, prefixed
`template:`, and are cherry-picked back.

Two things decide whether that stays smooth:

1. **Knowing which files are the template's**, so an edit that makes
   the next merge harder is noticed when it is made.
2. **Machinery that names no app resource.** Were sync, conflicts,
   pending changes, purge and a local user's move to `match` on
   `app_posts::TABLE`, `app_files::TABLE` and the rest, an app adding
   `recipes` or removing `posts` would edit every one of them, and the
   most delicate code in the template would be the worst merge
   conflict.

## Three kinds of files

| Kind | What | An app |
|---|---|---|
| **framework** | machinery an app uses as is: sync, events, conflicts, users, communities and memberships, the bytes of synced rows, accounts, purchasing, ops, `database.rs`, `deployment.rs`, `services/`, `helpers/`, the Svelte atoms, `loaded()`, `CommandError` | extends it, or changes it upstream |
| **seams** | files every app edits to add a feature: `tauri/src/lib.rs`'s command list, `axum/src/router.rs`, `svelte/src/lib/core/*`, the page list (`svelte/src/lib/pages.js`), the nav | edits them, a line per feature |
| **examples** | features that show the patterns: posts, and files as a resource (its name, search, duplicates, pages) | replaces or deletes them |

Ownership is recorded in a manifest, not in directory names. A
`framework/` directory cannot hold all of it without breaking the
per-app layout (`accounts/`, `ops/` and `data/accounts_*` are framework
too), and a directory should say what its code does, not who owns it.

## Decisions

### Sync dispatches through a registry of synced tables

The framework part of sync is the event queue, push, pull, cursors,
backfill, the conflict lifecycle and purge. The app's part is what each
table does: how an event applies, which rows a user sees, how a server
row is written on a device. A trait splits the two:

```rust
/// A synced table's part in sync, conflicts, pending changes, purge and
/// a local user's move to an account.
#[async_trait]
pub trait SyncedTable: Send + Sync {
    fn table(&self) -> &'static str;

    // On the server
    async fn apply_event(&self, deployment: &Deployment, user: &User, event: &Event)
        -> Result<Answer, SyncError>;
    async fn list_synced_after(&self, tx: &mut Transaction, user: &User, after: &Cursor, limit: usize)
        -> Result<Vec<TableRow>, SyncError>;
    /// The rows this one points at, sent before it on a pull.
    async fn parents(&self, tx: &mut Transaction, row: &TableRow) -> Result<Vec<TableRow>, SyncError>;

    // On a device
    async fn apply_server_row(&self, tx: &mut Transaction, user: &User, row: &TableRow)
        -> Result<bool, SyncError>;
    async fn delete_row(&self, tx: &mut Transaction, id: Uuid) -> Result<(), SyncError>;
    /// A local user's rows, as the create events their move to an account queues.
    async fn local_user_creates(&self, tx: &mut Transaction, user: &User)
        -> Result<Vec<LocalCreate>, SyncError>;

    // Conflicts and pending changes; a table that never shows its conflicts answers None
    async fn about(&self, tx: &mut Transaction, id: Uuid) -> Result<Option<About>, SyncError>;
    async fn reapply(&self, tx: &mut Transaction, role: Role, user: &User, event: &Event)
        -> Result<Answer, SyncError>;

    // Server only
    async fn purge(&self, tx: &mut Transaction, horizon: DateTime<Utc>) -> Result<u64, SyncError>;
}
```

The signatures are a sketch. They take their final shape as each `match`
is converted.

The registry is a single file the app owns, `core/src/app/synced_tables.rs`:

```rust
/// Every synced table, parents before the rows that point at them.
pub fn list() -> &'static [&'static dyn SyncedTable] {
    &[&users::Synced, &communities::Synced, &memberships::Synced, &posts::Synced, ...]
}
```

- **Why a trait and not one `match` file.** A `match` file would be
  simpler, but a table left out of one `match` fails at runtime, on a
  phone, in a sync that quietly drops a change. With the trait, the
  compiler checks that every registered table implements every
  operation. Decided 2026-11-21.
- **Order comes from the list.** The list order is the pull order,
  replacing `table_rank`. Purge runs it in reverse, children first,
  replacing `purge::TABLES`.
- **A table left out of the list** is the one mistake the compiler
  cannot catch. A test catches it instead: every `app_` table with a
  `global_sync_version` column must be registered.
- **Rows travel as `TableRow`** (table and JSON), ordered by their place
  in the pull, their `global_sync_version`. A pull sends each row after the
  rows it points at, asking each table for its `parents`.
- **Errors.** A table's own error reaches sync as one variant,
  `SyncError::Table`, a boxed source, so sync names no resource's error
  type. The web server answers it as a 500: a refusal travels inside
  the push's answer, not as an error.
- **`async-trait`**, already in `Cargo.lock` through a dependency.
  `dyn SyncedTable` needs boxed futures, which native `async fn` in
  traits does not give.

### Optional hooks, for what only some tables do

These are trait methods with defaults that do nothing:

- **Pruning on a device**: posts and files a user no longer sees.
- **Backfill on joining**: a live membership records its community to
  backfill, and each table lists its live rows in a community.
- **Bytes**: `fn bytes(&self) -> Option<&dyn SyncedBytes>`, for a
  table whose rows carry bytes, see below.
- **What goes with a user, a membership or a community**:
  `delete_of_user`, `delete_with_membership`, `revive_with_membership`
  and `delete_with_community`, so deleting an account, leaving or
  rejoining a community and deleting one name no table of the app's,
  see `examples-delete.md`.

### Communities are framework

Decided 2026-11-21. Users, communities and memberships are the
template's sharing model: what a user sees is decided by the communities
they are a live member of. Sync's backfill
(`BackfillRequest { community_id }`), visibility and pruning rest on it.
Being framework, they stay in sync rather than behind hooks.

The cost: an app with no communities of its own still has them, and
runs everything through one community that every user is in. Sign-up
already creates a user's first community. Making that a single shared
community, for such an app, is a suggestion for later, not part of this
work.

### Bytes are framework, files are an example

Decided 2026-11-21.

A file is two things:

- **The bytes of a synced row**: kept in the bucket and on the device
  apart from the row, checked by SHA-256, uploaded before the row's
  create is pushed, never dropped when refused (a device may hold the
  only copy), fetched on first view, cached, and cleaned up once no row
  points at them. This is the hard part, and any app with media wants
  it as is.
- **A file as a resource**: a name, its communities, search,
  duplicates, the Files pages. An app may have none of this, or videos
  and images as separate resources instead.

So the bytes are a capability any synced table can have, and `files`
is one example of a table that has it. An app with `app_videos`
and `app_images` gives each the capability and gets the same offline
handling, with no `files` table at all:

```rust
/// The bytes a table's rows carry, kept apart from the rows.
#[async_trait]
pub trait SyncedBytes: Send + Sync {
    /// The bytes of a row, from its create event: SHA-256, size, content type.
    fn bytes_of_create(&self, event: &Event) -> Result<Option<BytesOf>, SyncError>;
    /// Where the bytes live in the bucket.
    fn storage_key(&self, row: &TableRow) -> String;
    /// Whether the user may read the row's bytes, for a download URL.
    async fn readable_by(&self, tx: &mut Transaction, user: &User, id: Uuid) -> Result<bool, SyncError>;
    /// The ids of rows still live, so unreferenced bytes can go.
    async fn list_live_ids(&self, tx: &mut Transaction) -> Result<Vec<Uuid>, SyncError>;
}
```

The framework keeps the byte lifecycle in `core/src/sync/bytes.rs`
(today's `ask_about_waiting`, `held_back`, `upload_waiting`,
`local_copy`, the cache, `delete_unreferenced`, the refused-create rule)
and runs it over every registered table whose `bytes()` is `Some`.
Events that depend on a held-back row, as a file's community row does,
wait through `parents`, not through a check that names
`app_files_communities`.

- **Considered: one shared bytes table** that videos and images point
  at, like Rails' Active Storage blobs. Rejected: who may read a blob
  would depend on every row pointing at it, and a create would push two
  rows in order. With a capability, each table keeps its own columns
  and its own rules for who reads.
- **Avatars are not part of this.** They are online only, in a public
  bucket, and never synced.
- **As built**: a table opts in through `SyncedTable::bytes`. Keys in
  the bucket stay `<prefix>/<creator id>/<row id>`, the prefix the
  table's (`files` for files), and a device keeps bytes under the row's
  id in the account's folder. A device's sync asks
  `POST /api/sync/bytes/{table}/uploads` and fetches through
  `GET /api/sync/bytes/{table}/{id}/content`; the web's Files pages keep
  `/api/files/uploads` and `/api/files/{id}/content`, which call the
  same functions. The checks of a content type and a SHA-256 are in
  `helpers/`, each table's largest size in `SyncedBytes::max_bytes`.

### Sync is `core/src/sync/`

`sync`, `events`, `conflicts`, `pending_changes`, `purge`, `bytes` and
the `SyncedTable` trait are `core/src/sync/`, beside `database.rs` and
`deployment.rs`, the foundation. Callers name them there
(`app_core::sync::conflicts`), with no re-exports, as in
`code-organization.md`. `local_users` keeps its user-facing operations
in `app/`, and its move of rows goes through the registry.

`core/src/framework/` and `core/src/core/` were considered:

- `framework/` names ownership, not function, and cannot hold all of
  the framework (see above).
- `core/` reads badly as `app_core::core::sync`.

Axum and Tauri mirror it: `axum/src/sync.rs` and `axum/src/sync/`,
`tauri/src/sync.rs` and `tauri/src/sync/` with the conflicts and
pending changes commands.

A separate workspace crate (`app_sync`) would let the compiler refuse
any import of an example. It first needs `Deployment`, `database` and
`data/app_events` out of core, so it waits. The registry is what makes
that split possible later.

### Jobs follow the same pattern

The runner and the template's jobs are `core/src/jobs.rs`, each job a
`RecurringJob`; an app's own are listed in `core/src/app/jobs.rs`, as
with synced tables, and run after the template's. `app_jobs` keeps a
row per job by its name.

A task the server runs while it is up, a `ServerTask`
(`core/src/server_tasks.rs`), is listed the same way, in
`core/src/app/server_tasks.rs`; see "Server tasks" in `jobs.md`.

### The manifest and its check

Decided 2026-11-21: TOML, so a divergence carries its reason, and a
failed test, since declaring a divergence is one entry.

`framework.toml` at the root lists globs by kind, and the divergences an
app chose:

```toml
framework = ["core/src/sync/**", "core/src/accounts/**", "core/src/data/accounts_*/**",
             "svelte/src/lib/components/**", ...]
seams     = ["tauri/src/lib.rs", "axum/src/router.rs", ...]
examples  = ["core/src/app/posts.rs", "svelte/src/posts/**", ...]

[[divergence]]
path = "core/src/sync/sync.rs"
reason = "Larger pull pages for big files; upstream once settled"
```

- **`framework.lock`**, made in the template, holds each framework
  file's sha256. A test in `cargo test --workspace` fails when a
  framework file differs from its hash and has no `[[divergence]]`. It
  needs no git history or `template` remote, so it runs in CI too.
- **A clean template merge** updates `framework.lock` along with the
  files, so the check passes again by itself.
- **In the template**, the same command rewrites the lock (`--update`),
  and the test checks that the lock is current.
- **The divergences** are an app's backlog of changes worth sending
  upstream.
- **Seams and examples** are not checked. Seams are kept small by
  design, and examples are the app's to change.
- **An app's new files are its own**, wherever they sit. Since
  2026-11-23 a folder is claimed whole (`**`) only where an app adds
  nothing of its own (`core/src/sync/**`, `svelte/src/lib/components/**`);
  in `axum/src/`, `tauri/src/`, `core/src/services/`, `core/src/helpers/`,
  `svelte/src/lib/` and `bin/` the template's files are named one by one,
  so an app's `axum/src/app/recipes.rs` or `core/src/services/tmux.rs`
  is not reported as an added framework file. A test,
  `framework_check_leaves_an_apps_new_files_beside_the_templates_to_the_app`,
  keeps it so. The cost: a file the template adds there is listed in
  `framework.toml` too.
- **As built**: the `framework_check` crate. Its test,
  `framework_check_finds_the_framework_files_as_locked`, runs with the
  workspace's; `cargo run -p framework_check -- check` runs it alone and
  `-- lock` rewrites the lock. It walks the files itself, skipping
  `target`, `node_modules`, `dist` and `.git`, so it needs no git.
  `framework.lock` is one `<sha256>  <path>` a line, so a merge of two
  template updates conflicts only where both changed the same file.

### At the moment of the edit

Agents write much of the code, so the nudge comes when the edit is made:

- **AGENTS.md** gets a "Template and app code" section: read
  `framework.toml` before editing, prefer extending, and when a
  framework file must change, say so in the summary.
- **A Claude Code `PreToolUse` hook** warns on an edit to a framework
  path: extend it, declare a divergence, or change it upstream.

### Pulling down and contributing back

- **`bin/framework update`** fetches and merges `template/main`, runs
  the check, resolves a template change to an example the app
  deleted by keeping the deletion, and prints the entries of
  `FRAMEWORK_CHANGELOG.md` the merge brings in.
- **`bin/framework contribute`** lists the commits since the merge-base
  that touch framework files or start with `template:`, and
  cherry-picks the chosen ones onto a branch from `template/main`.
- **The version is the template commit.** A change that asks
  something of an app, or gives it something new to turn on, gets an
  entry in `FRAMEWORK_CHANGELOG.md` saying what changed and what an
  app does about it, its steps marked required or optional. An agent
  pulling the template reads the printed entries and takes the steps
  (AGENTS.md).

### Seams shrink when they hurt

A seam that keeps causing conflicts is split so the app's lines live in
an app-owned file:

- Tauri's `generate_handler!` takes the app's commands from a list the
  app owns.
- `app_api()` moves out of `router.rs`.
- The nav reads its entries from the app.

Each split happens once a seam has actually caused conflicts, not in
advance.

## Steps

1. `SyncedTable` and `app/synced_tables.rs`, converting one dispatch at
   a time, tests passing after each: push, pull and `with_parents`,
   device rows (`apply_row`, `delete_row`), conflicts, pending changes,
   purge, the local user's move, pruning and backfill. Files' byte
   handling stays as it is for now. Read `offline-sync.md` first.
2. The test that every synced table is registered.
3. Move to `core/src/sync/`, and mirror it in Axum and Tauri.
4. `SyncedBytes`: the byte lifecycle into `core/src/sync/bytes.rs`,
   `files` implementing it, and no `app_files` name left in sync.
5. The jobs registry.
6. `framework.toml`, `framework.lock` and the check.
7. The AGENTS.md section and the `PreToolUse` hook.
8. `bin/framework update` and `contribute`, and
   `docs/framework-changes.md`, now `FRAMEWORK_CHANGELOG.md`.
9. Update `code-organization.md`, `code-organization-axum-tauri-svelte.md`,
   `offline-sync.md`, `offline-files-decisions.md` and
   `files-communities.md`.

