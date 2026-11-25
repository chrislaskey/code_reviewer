# Deleting the examples

Status: built 2026-11-25, steps 1 to 5; step 6 open.

## Request

`template-and-apps.md` says posts and files are examples, an app's to
replace or delete. In practice an app could take them out of the seams,
but could not delete them: the framework still named them. Found
2026-11-23 moving Code Reviewer onto the template, and again 2026-11-25
starting Emoji Status, which keeps files and wants posts gone. Both
hid the examples instead, keeping their core modules, tables and
listing in `synced_tables.rs`.

What should hold: an app deletes an example's files and its lines in
the seams, and everything else builds and passes, with no
`[[divergence]]`.

## Where the framework named the examples

Code, outside tests:

- `accounts/deletions.rs`: deleting an account deletes the user's posts
  through `posts::apply_in`. `DeletionError` had a `Post` variant.
- `app/memberships.rs`: leaving a community calls
  `posts_communities::delete_with_membership`, and rejoining calls
  `posts_communities::revive_with_membership`.
- `app/communities.rs`: deleting a community calls
  `files_communities::delete_with_community`.
- `svelte/src/conflicts/ConflictsIndexPage.svelte` links "Back to posts"
  through `postsIndexUrl`.
- `svelte/src/communities/components/CommunitiesDeleteDialog.svelte`
  says what happens to the posts in a deleted community.

Tests: about 250 references in 14 framework files, 117 of them in
`core/src/sync.rs`. They are two kinds:

- **Tests of the machinery**: batching, retries, conflicts, cursors,
  purge, resync, a local user's move, bytes. Posts are just a handy
  synced row.
- **Tests of an example's own rules**, sitting in a framework file: a
  post taken out of a community comes back on rejoining, a file
  outlives its creator's account.

## Decisions

### Each table decides what happens to its rows

The three calls are rules that differ by table. A post is its writer's:
it leaves with their membership and goes with their account. A file is
its communities': it stays when its creator leaves, and goes when its
last community is deleted. So each is a `SyncedTable` method with a
default that does nothing, as `move_rows_to_community` and `prune` are:

```rust
/// The user's rows deleted with their account, as events in their name.
async fn delete_of_user(&self, tx: &mut Transaction, role: Role, user: &User)
    -> Result<(), TableError>;
/// The rows under a membership deleted with it, at its moment.
async fn delete_with_membership(&self, tx: &mut Transaction, role: Role, membership: &Membership)
    -> Result<(), TableError>;
/// The rows that left with a membership revived with it.
async fn revive_with_membership(&self, tx: &mut Transaction, role: Role, membership: &Membership,
    left_at: DateTime<Utc>) -> Result<(), TableError>;
/// The rows in a community deleted with it, at its moment.
async fn delete_with_community(&self, tx: &mut Transaction, role: Role, community: &Community)
    -> Result<(), TableError>;
```

- The framework calls each on every listed table. Deletes run children
  first, as purge does; revives run parents first.
- Posts implement `delete_of_user`. Posts' community rows implement the
  two membership hooks. Files' community rows implement
  `delete_with_community`.
- A table's error reaches its caller as a `Table` variant, a boxed
  source, as in `SyncError`.
- A trial user's delete needs no hook. It deletes their `app_users` and
  `app_communities` rows outright, and an app's table goes with them by
  `ON DELETE CASCADE`.

### The pages name no example

The conflicts page links back to Home. The community delete dialog
says what happens without naming posts.

### A test keeps it so

A test in `framework_check`,
`framework_check_finds_no_example_named_in_the_framework`, reads every
framework file and fails on a name of an example, outside comments:

- In Rust: a path (`posts::list`, `app::files`, a name in a `use`), a
  table (`app_posts`) or an API path (`"/api/posts"`).
- In the pages: a name joined to another word (`postsIndexUrl`,
  `PostsForm`), a component (`FileImage`) or a part of an import path
  (`"../files/FilesForm.svelte"`).

The names come from the `examples` list of `framework.toml`, each
glob's last plain part: `core/src/app/posts.rs` is `posts`. A name is
looked for in its example's language, those under `svelte/` in the
pages and the rest in Rust, so `navigator.clipboard` in a page is not
the Tauri `clipboard` example. A word in a sentence is not looked for:
`files` is a common variable. An app's own examples are checked too.

Considered: a test that deletes the examples in a copy of the repository
and builds it. Rejected: it would have to edit the seams, which every
app changes, so it would break on the app's own lines.

It reads the tests too, so a framework test cannot lean on an example
either.

### The framework tests on a table of its own

Decided 2026-11-25. The machinery's tests use a table compiled for
tests alone, `app_test_notes`, in place of posts and files. An app
that deletes the examples keeps every test of its sync, which matters
most to an app with a `[[divergence]]` there.

- It is small: a note with a body, in one community or none, its
  `community_id` on the row rather than in a join table. The body and
  the delete are two fields that can conflict. That covers conflicts,
  visibility by community, backfill, pruning, purge, a local user's
  move and an account's delete, without any example's rules.
- It implements every hook: it goes with its owner's account, with its
  community, and with its owner's membership there, which rejoining
  brings back, by its `deleted_at` matching the membership's.
- It is built with `#[cfg(any(test, feature = "test-tables"))]`. Axum
  and Tauri turn the feature on as a dev-dependency, so a build that
  ships has none of it.
- Its table is created by `CREATE TABLE IF NOT EXISTS` after the
  migrations, whenever a test opens a database, and not by a migration:
  sqlx refuses a database whose applied migrations it does not all
  know, and a test may open a file again.
- The framework lists it after the app's tables, in `sync::tables`,
  which every part of sync calls. `app/synced_tables.rs` keeps only
  the app's list.
- The axum tests' `test_router()` makes no post. A test that wants one
  makes it, as the seam's `test_router_with_a_post()` does.
- Not yet: bytes. The bytes machinery's tests are in the files example,
  `core/src/app/files.rs`, so an app deleting files loses them. Moving
  them onto test notes with bytes is a later step.

Considered: moving every test that uses posts or files into the
examples. It is simpler, but an app that deletes them loses most of its
sync tests.

### An example's tests stay with it

The tests of an example's own rules are in its files, so they go when
it goes: posts' community rows taken out and brought back, a file
outliving its creator's account, a local user's file moving with its
bytes. The files example's tests use test notes, not posts, for a row
beside a file.

A test against a running web server needs a table the server has, so
the framework's (`sync_two_devices_over_http`) syncs a community, and
the files example keeps its own for bytes.

### A deleted example leaves no unused framework code

An app builds with `-D warnings`, so a framework function only an
example called would fail its lint once the example went.
`users::list_by_ids` is `pub`, for an app's resources to show who
made a row, and Tauri's `user_auth::signed_in_with_transport` allows
being unused, for an app's commands that ask the server.

Checked 2026-11-25 by deleting both examples in a copy of the
repository: `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo test --workspace --all-targets`, `pnpm check`, `pnpm build` and,
after `pnpm translations:update`, `pnpm translations:check` all passed.

### Migrations

An example's tables are created by its own migrations, listed in
`examples`. An app with no deployed database deletes them with the
example, and resets its local databases. An app with a deployed
database keeps them and adds a migration that drops the tables:
sqlx refuses a database with an applied migration that is missing.

## How an app deletes an example

1. Delete the example's files: the globs of `examples` in
   `framework.toml` for it, its migrations among them, and then those
   globs.
2. Take its lines out of the seams:
   - `core/src/app.rs` and `core/src/data/mod.rs`: its modules.
   - `core/src/app/synced_tables.rs`: its tables.
   - `axum/src/app.rs`, `axum/src/router.rs`: its module and routes.
   - `axum/src/api_error.rs`, `tauri/src/command_error.rs`: its errors.
   - `axum/src/tests/app.rs`, `axum/src/tests/router.rs`: its tests,
     and `test_router_with_a_post` for posts.
   - `tauri/src/app.rs`, `tauri/src/lib.rs`: its module and commands.
     For files also the `files` URI scheme, `quick_picker::register`,
     and the `clipboard`, `macos` and `quick_picker` modules.
   - `svelte/src/lib/pages.js`, `routes.js`, `layouts/AppNav.svelte`
     and `core/`: its pages, paths, nav item and calls. For files also
     the `quick_picker` input of `svelte/vite.config.js`.
3. Run `pnpm translations:update` in `svelte/`.
4. Reset the local databases, `bin/reset`, since their applied
   migrations name the deleted ones.
5. Run the checks of AGENTS.md, section 2.

No `[[divergence]]` is needed: none of these is a framework file.

## Steps

1. The hooks, with posts and files implementing them.
2. The pages.
3. The check, of framework code outside tests. A changelog entry for
   steps 1 to 3.
4. `app_test_notes`, and the machinery's tests moved onto it. A
   changelog entry.
5. The examples' own tests moved into the examples, and the check
   covering tests too. A changelog entry saying how to delete an
   example.
6. Bytes on test notes, and the bytes machinery's tests moved from
   `files.rs` onto them.
