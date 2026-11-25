# Deleting the examples

Status: planned 2026-11-25; steps 1 to 3 built 2026-11-25.

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

Until the tests are moved (below), the check skips each file's tests.

### The framework tests on a table of its own

Decided 2026-11-25. The machinery's tests use a table compiled for
tests alone, `app_test_notes`, in place of posts and files. An app
that deletes the examples keeps every test of its sync, which matters
most to an app with a `[[divergence]]` there.

- It is small: a note in a community, with a title and a body, so two
  fields can conflict, and optional bytes. That covers conflicts,
  visibility by community, backfill, pruning, purge, a local user's
  move and bytes, without any example's join table.
- It is built with `#[cfg(any(test, feature = "test-tables"))]`. Axum
  and Tauri turn the feature on as a dev-dependency, so it never ships.
- Its migration is the tests' own, run on their in-memory database
  after the app's.
- It is listed for tests by the framework, after the app's tables, so
  `synced_tables.rs` stays the app's.

Considered: moving every test that uses posts or files into the
examples. It is simpler, but an app that deletes them loses most of its
sync tests.

### An example's tests stay with it

The tests of an example's own rules move into its files, so they go
when it goes. The files example's tests stop creating posts.

### Migrations

An example's tables are created by its own migrations, listed in
`examples`. An app with no deployed database deletes them with the
example, and resets its local databases. An app with a deployed
database keeps them and adds a migration that drops the tables:
sqlx refuses a database with an applied migration that is missing.

## Steps

1. The hooks, with posts and files implementing them.
2. The pages.
3. The check, of framework code outside tests. A changelog entry for
   steps 1 to 3.
4. `app_test_notes`, and the machinery's tests moved onto it.
5. The examples' own tests moved into the examples, and the check
   covering tests too. A changelog entry saying how to delete an
   example.
