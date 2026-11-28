# Framework changelog

What the template changed that an app built from it should know about,
newest first: what an app must change to keep working, and what it can
now turn on. The template's version is its commit; there are no
releases. A change that asks nothing of an app and gives it nothing new
is not listed: `bin/framework update` merges it. See
`docs/design/template-and-apps.md`.

`bin/framework update` prints the entries a merge brings in. Read each
one and do the steps under **What an app does** that apply to the app:
**Required** steps keep it working, **Optional** ones are for an app
that wants the new thing. Say in the summary which were done and which
were skipped, and why.

Each entry: the date and a title, the commits, **What changed**, and
**What an app does**, "Nothing" when nothing. In the template, add one
in the commit that makes such a change, naming the earlier commits it
completes.

## 2026-11-27: the purge no longer holds the feed lock while it deletes

Commits: this one.

- **What changed**. `sync::purge::run` holds the feed lock only to
  remove the old change log, move the floor and record the run. The
  tombstones go after, a table at a time, without it, so writers don't
  wait on them. `docs/design/offline-sync.md` gains the rule for hard
  deletes: a synced row is deleted outright only when no device can
  hold it.
- **What an app does**.
  - **Required**, for an app's own hard delete of a synced row: check it
    against the rule, or soft-delete the row instead.

## 2026-11-27: app_web sync feed renew, after a restore

Commits: this one.

- **What changed**. `app_web sync feed renew`, and
  `app_core::sync::feed_id_renew`, give the server's sync feed a new id.
  Run it after restoring the database from a backup, before people sync
  again: every device resyncs at its next pull. Without it a device can
  miss changes, since the restored database may give out again versions
  it already saw.
- **What an app does**.
  - **Optional**. Add the command to the app's restore steps, as the
    README's Fly.io section does.

## 2026-11-27: device sign-in, resent codes and terms through a remote backend

Commits: this one. Step 3 of `docs/design/backend-seam.md`.

- **What changed**.
  - Beside a remote backend, the desktop and phone apps sign in,
    a code can be resent, and the last step records the terms
    accepted: `accounts::sign_in_confirm_for_device`,
    `sign_in_with_device_code`, `resend_code_to` and `resend_code` call
    it, and `sign_in_confirm` sends `TERMS_VERSION`.
  - `trial_users::count` and `create` call the remote backend, which
    the sample answers `not_implemented`, so a guest visit fails plainly
    there until a backend has trials. `trial_users::create` with a cap
    of 0 is refused before it opens a transaction, on every backend, and
    `GET /api/accounts/trial` counts nothing at a cap of 0.
  - The conformance suite has three more tests, ten in all.
  - `core_elixir/` has `mix core.feed_id_renew`, for after a restore.
- **What an app does**. Nothing.

## 2026-11-27: a conformance suite for remote backends, and Elixir in bin/lint

Commits: this one. Step 2 of `docs/design/backend-seam.md`.

- **What changed**.
  - `core/src/app/files/conformance_tests.rs`, with the files example:
    seven tests of sync, files and
    memberships, run on the Rust backend with the other tests, and
    against a remote backend with `CONFORMANCE_BACKEND_URL` and
    `--ignored`. `core_elixir/` passes them all.
  - `bin/lint` checks `core_elixir/`'s format and compile while it is
    there, the `elixir` target.
  - `core_elixir/` reads its dev port from `BACKEND_PORT`, not `PORT`,
    which is the web server's.
- **What an app does**.
  - **Optional**, for an app that deleted `core_elixir/`: nothing;
    `bin/lint` skips it.
  - **Optional**, for an app with its own remote backend: run the suite
    against it, and add a test for each resource it serves.

## 2026-11-27: the web server can call a remote backend

Commits: `52bb028`, `25cf524`, which dispatches sign-in and the
current user, `869193b`, which dispatches sync, `991d995`, which
dispatches communities, memberships and files, `5be5419`, which runs
the bucket sweep beside a remote backend, and the commit after it, which
gives `bin/dev` `BACKEND=elixir`. The first parts of step 1 of
`docs/design/backend-seam.md`.

- **What changed**.
  - `BACKEND=remote`, with `BACKEND_URL` and `BACKEND_SERVICE_TOKEN`,
    makes the web server call a remote backend, such as
    `core_elixir/`, which owns the database. The server then opens no
    database. `BACKEND` unset, or `rust`, is the server as before.
  - Core's `remote-backend` feature has the client,
    `services::backend::RemoteBackend`. `Deployment` has
    `remote_backend`, and `Database` has `None`.
  - The token check, sign-in's three steps, sign-out, the current
    user's account, sync's push, pull and backfill, the communities
    list, joining and leaving, and files call it. Reading a row's
    bytes asks `<resource>/readable`, named from the table without
    `app_`.
  - `RecurringJob` has `runs_beside_a_remote_backend`, false unless a
    job says so. Beside a remote backend only those run, the bucket
    sweep of files among them, due by when the process last ran them.
  - `BACKEND=elixir bin/dev` runs the web over `core_elixir/`, on the
    web port + 3000, see `bin/dev --help`. In remote mode, staff, plans, preferences
    and trials answer nothing, since the sample doesn't serve them.
    Every other entry fails in remote mode, until its area is
    dispatched.
- **What an app does**.
  - **Required**, for an app whose code matches on `Database`: add an
    arm for `Database::None`.
  - **Required**, for an app's code that matches on `UserError`,
    `AccountError`, `AccountUserError`, `SyncError`, `BytesError`,
    `CommunityError`, `MembershipError` or `FileError`: handle their
    `Backend` variant.
  - **Optional**, for an app's own job on what Rust keeps, such as a
    bucket, when the app will use a remote backend: answer true from
    `runs_beside_a_remote_backend`.
  - **Optional**. Nothing else changes for an app on the Rust backend.

## 2026-11-27: every rejection is shown to the person, and a refused create is kept

Commits: this one.

- **What changed**.
  - A create the server refuses, with no row of its own to send, is kept
    on the device as a conflict with the reason, for every synced table.
    Before, only a table with bytes kept it. Any other table deleted the
    row and its events without a word.
  - The row stays. Its later events, and the creates of rows pointing at
    it by a foreign key of `SyncedTable::foreign_keys`, wait with it,
    held back from each push.
  - A resync keeps it. The conflicts page offers Try again and Let go;
    letting it go removes the row and the rows waiting with it.
  - An update the server rejects is refused at once, not tried 30
    times. When the server sends its row, the row wins and the person's
    change is kept as a refused conflict, to reapply or let go; before,
    it gave way without a word. Only a file waiting for its bytes still
    counts tries.
  - `app_sync_state::maintenance::delete_not_pending` is now
    `delete_not_pending_or_refused`.
- **What an app does**.
  - **Required**, for an app's own synced table that points at another
    synced row: list that column in `foreign_keys`, so its creates wait
    with a refused parent. Most already do, for resync and purge.
  - **Required**, for an app's own table that overrides
    `delete_for_resync`: keep the rows whose create was refused, an
    `app_events` row with `status = 'conflict' AND type = 'create'`, as
    `app_files`'s does. A table on the default needs nothing.
  - **Optional**. Mark a refused row in the app's own pages, as the files
    pages do. The conflicts page already lists it.

## 2026-11-27: a sample Elixir backend replaces phoenix/

Commits: `b11afb9` removed `phoenix/`, `969ef0d` added `core_elixir/`,
`9a92088` moved its HTTP layer to Phoenix, and the commit after it
named it `core` and gave it the Rust PostgreSQL migrations.

- **What changed**. `phoenix/`, the earlier reference app, is gone.
  `core_elixir/`, the Elixir app `core`, is a sample remote backend:
  Elixir with Phoenix,
  PostgreSQL, Ecto, Oban and Commanded, answering the operations of
  `docs/design/backend-protocol.md`. Files are its synced resource.
  Its schema is the Rust core's PostgreSQL migrations, which it runs
  itself. Axum doesn't call it yet, see `docs/design/backend-seam.md`.
  It is an example, so `framework.toml` never checks it.
- **What an app does**.
  - **Optional**. An app on the Rust backend deletes `core_elixir/`. An
    app that will use an Elixir backend keeps it, and builds from it.
  - **Optional**. Delete a `phoenix/` the merge left, if the app changed
    files inside it.

## 2026-11-27: global sync versions replace timestamp cursors

Commits: `5861a22`, `53a21ec`, and the commit after it that splits
`SyncedTable`.

- **What changed**.
  - The server gives each change to a synced row a `global_sync_version`.
    The version commits in the write's own transaction, with an
    `app_sync_changes` entry. A device pulls through the versions,
    not the server's clock. Sync protocol 2 replaces protocol 1, and
    the baseline schemas changed.
  - A data context's `create` and `update` for a synced table take the
    `Role`, stamp the row, and return it. On the server they publish
    the change; on a device they keep the row's version.
  - A pull pages by the rows the user sees. Other users' changes cost
    no round trips.
  - An interrupted first pull, or community backfill, continues from
    its last page on the next sync.
  - A retried event is answered before its transaction waits for the
    feed lock. A sign-in that writes no synced row takes no feed lock.
  - The core sync tests fail on any server row written without
    publishing its change, `assert_every_row_published`.
  - `SyncedTable` keeps what a device and the server both use. The
    Rust server's pull reads, `list_synced_after`, `parents` and
    `list_live_in_community_after`, move to a new `SyncedTableServer`
    trait. A table answers `server()` with it, or with none for a table
    a remote backend serves.
  - `device_settings::upgrade` moved from `data::device_settings` to
    `core/src/device_settings.rs`. It opens its own transaction there.
- **What an app does**.
  - **Required**. For each synced table of the app's own:
    - In its create migration, add `global_sync_version INTEGER CHECK
      (global_sync_version > 0)` (`BIGINT` on PostgreSQL) and an index
      on it. Drop the `(last_synced_at, id)` index.
    - Add the field to its struct, with
      `#[serde(with = "crate::sync::version::optional")]`, and to its
      SQL.
    - Give its data context's `create` and `update` the `Role`. Stamp
      the row with `app_sync_changes::stamp` before writing it, and
      return it, as `data/app_files` does. Make the write paths call
      them, and stamp nothing by hand. Direct SQL that changes synced
      rows on the server records one change per row, as
      `trial_users::maintenance` does.
    - In its `apply_in`, answer a retried event first. Then call
      `app_sync_versions::lock_for_role` before reading the row it
      decides on.
    - Make `list_synced_after` and `list_live_in_community_after` take a
      `&VersionRange`. Select `global_sync_version > after AND
      global_sync_version <= through`, ordered by it. Declare
      `foreign_keys()` on its `SyncedTable`.
    - Move `list_synced_after`, `parents` and
      `list_live_in_community_after` from its `impl SyncedTable` to an
      `impl SyncedTableServer` for the same struct. Add
      `fn server(&self) -> Option<&dyn SyncedTableServer> { Some(self) }`
      to its `impl SyncedTable`, as `app/files.rs` does.
  - **Required** where the app calls it: call
    `app_core::device_settings::upgrade` in place of
    `app_core::data::device_settings::upgrade`.
  - **Required**. Remove any divergence for these files that the
    framework's version now covers.
  - **Required**. Stop the development server and desktop app. Run
    `bin/reset` for the development checkout and database, then
    restart. This deletes local account databases, pending changes
    and device files. It also recreates the configured development
    PostgreSQL database when it can be reached. Use `bin/reset --dry-run`
    to see its targets first. Clear the data on test phones, simulators
    and other machines separately. Then install builds that match the
    server.
  - **Verify**. Run the app's sync tests through the core `sync` test
    helper, which checks that every row is published. Sign in on two
    clean devices. Make an offline edit and see it on the other device
    after reconnecting. Check that an online edit, a deletion and a
    change in community access also reach the devices.

## 2026-11-26: restore the iPhone app layout after login

- **What changed**: after a successful return from the iPhone's sign-in
  sheet, the native plugin handles the callback on the main queue and
  restores the app webview's layout before returning the code. This
  fixes a white screen after login with a zero-size viewport, even
  though the account was saved and the Home page had loaded.
- **What an app does**:
  - **Required**, for an iOS app: rebuild and reinstall the native app.
    Updating the frontend alone does not include the Swift fix.
  - **Required**, if the app applied the temporary diagnostic or its
    own copy of this fix: remove its `[[divergence]]` for
    `tauri/plugins/web-auth-session/ios/Sources/WebAuthSessionPlugin.swift`
    and use the template's version without the diagnostic logs. Remove
    any temporary Accounts-page navigation override and its divergence.
  - **Verify** successful login opens the app at full size, cancelling
    the sheet returns to Accounts, and another login still succeeds.

## 2026-11-25: the dev database and buckets named by the app

- **What changed**: `bin/reset` and `bin/storage` name what they make
  after the `productName` in `tauri/tauri.conf.json`, as `bin/reset`
  already names the app data directory after its `identifier`.
  `bin/reset` drops the PostgreSQL database `starter_pack` for
  "Starter Pack", when `DATABASE_URL` names none. `bin/storage` makes
  the buckets `starter-pack` and `starter-pack-private` and the key
  `starter-pack-dev`. So two apps on one computer no longer share a dev
  database. The template's names are as before.
- **What an app does**:
  - **Optional**, for an app with its own `productName` and a dev
    PostgreSQL database: make it under the new name, such as
    `createdb emoji_status` for "Emoji Status", and point `DATABASE_URL`
    at it. Drop the old `starter_pack` database once no app uses it.
  - **Optional**, for an app with its own `productName` and files in
    Garage: the next `bin/storage start` makes the new buckets, empty,
    and the files in the old ones are no longer read. Run `bin/reset`,
    and delete `axum/data/garage/` to drop the old buckets.

## 2026-11-25: an app deletes the examples

- **What changed**: an app can delete posts, files or both, see "How an
  app deletes an example" in `docs/design/examples-delete.md`. The
  framework's tests name no example: the tests of the examples' own
  rules moved into them (`posts_communities.rs`, `files.rs`,
  `files_communities.rs`, and `files_uploads_are_refused_to_a_trial_user`
  in `axum/src/tests/app.rs`), and
  `framework_check_finds_no_example_named_in_the_framework` reads tests
  too. `sync_two_devices_over_http` syncs a community; the files
  example's over-HTTP tests are `files_a_file_over_http` and
  `files_many_files_over_http`. `users::list_by_ids` is `pub`, and
  Tauri's `user_auth::signed_in_with_transport` allows being unused, so
  a deleted example leaves no dead framework code. `framework.toml`
  lists the quick picker's page, `svelte/src/quick-picker/**` and
  `svelte/src/quick-picker.html`, and `tauri/capabilities/quick-picker.json`
  as the files example's, and `core/src/app.rs` and
  `core/src/data/mod.rs` as seams.
- **What an app does**:
  - **Optional**, for an app that hid an example, keeping its core
    modules, its table and its line in `synced_tables.rs` for the
    framework's sake: delete it, by the design's steps, and reset the
    local databases (`bin/reset`). Remove any `[[divergence]]` it
    declared for a framework file it changed to do so.
  - **Required**, for an app that changed the moved tests in its copy
    of `sync.rs`, `memberships.rs`, `local_users.rs` or `deletions.rs`:
    keep the template's, and carry its own changes over to the
    example's files.

## 2026-11-25: the framework's tests on a table of its own

- **What changed**: the framework's tests use `app_test_notes`, a synced
  table built for tests alone (`core/src/sync/test_notes.rs`), in place
  of posts, see `docs/design/examples-delete.md`. It is behind
  `#[cfg(any(test, feature = "test-tables"))]`, a new core feature that
  `axum/Cargo.toml` and `tauri/Cargo.toml` turn on in
  `[dev-dependencies]`. Sync runs over `sync::tables::list()`, the
  app's `synced_tables::list()` and the test table after it in tests;
  `synced_tables::get` and `position` are now `sync::tables::get` and
  `position`, and the test that every table with sync columns is
  listed moved there too. The axum tests' `test_router()` and
  `test_router_with()` return the `TestRouter` alone, with no post;
  `axum/src/tests/app.rs` has `test_router_with_a_post()`.
- **What an app does**:
  - **Required**: keep `app_core = { workspace = true, features =
    ["test-tables"] }` in the `[dev-dependencies]` of `axum/Cargo.toml`
    and `tauri/Cargo.toml` when the merge conflicts.
  - **Required**, for an app whose `core/src/app/synced_tables.rs`
    conflicts: keep its own `list()`, and drop `get`, `position` and
    the test below it, which are in `sync::tables` now. Code of the
    app's that called `synced_tables::get` or `position` calls
    `sync::tables::get` or `position`.
  - **Required**, for an app with axum tests of its own: write
    `let router = test_router().await;` for
    `let (router, _) = test_router().await;`, and use
    `test_router_with_a_post()` where a test used the post.

## 2026-11-25: the framework names no example

- **What changed**: the framework's code no longer names posts or
  files, see `docs/design/examples-delete.md`. What goes with a deleted
  account, a left or rejoined membership and a deleted community is
  each table's own, by four `SyncedTable` methods with defaults that do
  nothing: `delete_of_user`, `delete_with_membership`,
  `revive_with_membership` and `delete_with_community`. Posts implement
  the first, posts' community rows the two membership ones, and files'
  community rows the last. `DeletionError::Post` is now
  `DeletionError::Table`, and `MembershipError` and `CommunityError`
  have a `Table` variant. The conflicts page links back to Home. A test,
  `framework_check_finds_no_example_named_in_the_framework`, fails on a
  framework file naming an example outside its tests. `framework.toml`
  lists `svelte/src/posts/**` and `svelte/src/files/**` as examples.
- **What an app does**:
  - **Required**, for an app that matches `DeletionError`,
    `MembershipError` or `CommunityError` in its own code: answer the
    `Table` variant, as `axum/src/api_error.rs` does.
  - **Required**, for an app that changed `core/src/app/posts.rs`,
    `posts_communities.rs` or `files_communities.rs`: keep the new
    `SyncedTable` methods in its copy when the merge conflicts, or a
    deleted account keeps its posts, and a left membership its posts'
    community rows.
  - **Optional**: a table of the app's own whose rows go with their
    maker's account, leave with a membership, or go with a community
    implements the method.

## 2026-11-25: the theme per person

- **What changed**: the theme is a preference, `theme` in
  `accounts_preferences`, beside `locale`: per person, so each account
  on a device has its own, synced to their other devices. Its values are
  `helpers::themes::SUPPORTED`, which a core test keeps in step with
  `THEMES` in `svelte/src/lib/layouts/themes.js`, or null for "System".
  `Preferences` has a `theme` field. The theme menu writes it, and the
  pages follow it with `themesFollow` when the current user loads.
  `theme_script.js` keeps the last theme shown on the device for the
  first paint, takes `app:set-theme` with the theme in `detail.theme`,
  and dispatches `app:theme-changed`. A theme chosen before this change
  on a device is replaced by the person's preference, "System" until
  they choose. See `docs/design/locales.md`, "The theme".
- **What an app does**:
  - **Required**, for an app that changed the themes in `themes.js` or
    `theme.css`: list the same ones in `core/src/helpers/themes.rs`,
    declared as a `[[divergence]]`.
  - **Required**, for an app that dispatches `app:set-theme` itself:
    call `themesFollow` instead.
  - **Required**, for an app that builds `Preferences` field by field:
    add `theme: None`.

## 2026-11-25: a file's content on its row

- **What changed**: a row with bytes may keep small text content on the
  row instead, with nothing in the bucket, see
  `docs/design/files-content.md`. `SyncedBytes` has two methods for it,
  with defaults: `content_of_create`, a create's `content` field, and
  `content`, none. A create that carries `content` does not wait for an
  upload, `bytes::local_copy` writes the content on a device with no
  fetch, and `bytes::row_content` answers it on the server, which
  `GET /api/sync/bytes/{table}/{id}/content` now sends itself, sandboxed
  (`content_response` in `axum/src/sync/bytes.rs`). The files example
  uses it: `app_files.content`, by the migration
  `20261125000000_app_files_content_add.sql`, `content` on `FileNew`,
  with `size` and `sha256` worked out from it, at most 64 KB, and
  `files::row_content`, which `GET /api/files/{id}/content` answers.
- **What an app does**:
  - **Required**, for an app that kept the files example: keep the
    migration, and add `content: None` to any `File` or `FileNew` it
    builds field by field.
  - **Required**, for an app that changed `app_files` itself: merge the
    example's changes into its own, or delete the migration if it has
    no `app_files`.
  - **Optional**: a table of the app's own with bytes keeps content on
    its rows by a `content` field in its create and by answering
    `SyncedBytes::content`.
## 2026-11-24: Flash messages

- **What changed**: flash messages are short notices above the page,
  such as "Switched to Alice" or an error. Call
  `flashMessageShow(text, { tone, icon })` from
  `svelte/src/lib/flash_messages.svelte.js`.
  `svelte/src/lib/layouts/FlashMessages.svelte` draws them as daisyUI
  alerts. Several can show at once. Each stays until it is closed or the
  next navigation starts. `Alert` has a new `success` tone.
  `page_load_error.svelte.js` is gone: the router shows a page that
  could not load as an error flash message. The double tap on the avatar
  says who it switched to, and shows its error. The "account was
  deleted" notice is a flash message too, and `accountsIndexUrl` no
  longer takes `{ accountDeleted }`; the `?account_deleted` query is gone.
- **What an app does**:
  - **Required**, for an app that calls `pageLoadErrorShow` or
    `pageLoadErrorClear`: call `flashMessageShow(text, { tone: "error" })`
    or `flashMessagesClear()` instead.
  - **Required**, for an app that calls `accountsIndexUrl({ accountDeleted: true })`:
    call `accountsIndexUrl()`, then show the notice with `flashMessageShow`
    once the navigation finishes.
  - **Optional**: show the app's own notices with `flashMessageShow`, see
    README, Flash messages.

## 2026-11-24: A switch to an account opens its last visited page

- **What changed**: in the app, the router keeps each page loaded as
  the active account's `last_visited_page` device setting. The device's
  own pages, such as `/accounts`, are not kept. A switch to an account
  opens that page again: Switch on `/accounts`, or a double tap on the
  avatar, which switches to the next signed-in account (commit
  `5201fc5`). Signing in and starting the app still land on Home.
  `deviceAccountsSwitch` now answers `{user, last_visited_page}` instead
  of the user. `navigateToUrl` now keeps a URL's query.
- **What an app does**:
  - **Required**, for an app that calls `deviceAccountsSwitch` and uses
    what it answers: read the user from its `user`.

## 2026-11-24: bin/dev creates .env from .env.example

- **What changed**: `bin/dev` copies `.env.example` to `.env` when there
  is no `.env`, as in a new checkout or worktree, and says so. It never
  changes a `.env` that exists.
- **What an app does**:
  - **Required**, for an app that declared a `[[divergence]]` for
    `bin/dev` to do this: remove it from `framework.toml`.
  - **Optional**: keep `.env.example` ready to run as it is, since a new
    worktree's `bin/dev` now starts from it.

## 2026-11-24: Wi-Fi sync icons, no name in the header, a closer page top

- **What changed**: the header's sync status is a Wi-Fi icon online and
  a Wi-Fi-off icon offline, from Lucide: `lucide-static` and
  `svelte/vendor/lucide.js`, the `lucide-*` classes for the icons
  heroicons lacks, are the template's now. The app's name in the header
  on a single-community server is gone again, and with it
  `svelte/src/lib/identity.js`; a test still keeps the `<title>` of
  `svelte/src/index.html` equal to `APP_NAME`. On small windows a page
  starts closer below the header.
- **What an app does**:
  - **Required**: delete `svelte/src/lib/identity.js`, if the merge
    left it.
  - **Required**, for an app that had added Lucide itself: keep the
    template's `svelte/vendor/lucide.js`, the `@plugin` line in
    `svelte/src/app.css` once, and `lucide-static` in
    `svelte/package.json` once, then `pnpm install`.

## 2026-11-24: the app's name in the header on a single-community server

- **What changed**: on small windows, a server with a single community
  shows the app's name in the header, a link to Home, where the
  community menu would be. The pages read the name from the new seam
  `svelte/src/lib/identity.js`, `APP_NAME`; a test of the Tauri app
  keeps it, and the `<title>` of `svelte/src/index.html`, equal to
  `APP_NAME` in `core/src/app/identity.rs`.
- **What an app does**: **Required**: set `APP_NAME` in
  `svelte/src/lib/identity.js` and the `<title>` in
  `svelte/src/index.html` to the app's name; `cargo test --workspace`
  fails until they agree.

## 2026-11-24: no bounce on the desktop, a see-through header on small windows

- **What changed**: the desktop app's window no longer bounces past the
  page's ends (`overscroll-behavior: none`), as the phone's doesn't.
  Below 40rem the header, which scrolls away with the page, has no
  background and no line under it, so the page's background shows
  through it.
- **What an app does**: **Optional**: an app with a background of its
  own at the top of its pages, such as a pattern, may start it at the
  window's top below 40rem, behind the header and a phone's status bar.

## 2026-11-24: the sign-in scheme checked against the app's identifier

- **What changed**: a test of the Tauri app fails when `APP_URL_SCHEME`
  in `svelte/src/lib/routes.js` differs from `APP_IDENTIFIER` in
  `core/src/app/identity.rs`. Different, the iPhone's sign-in page
  showed the code to paste instead of returning to the app.
- **What an app does**: **Required**: set `APP_URL_SCHEME` to the app's
  identifier, the bundle identifier in `tauri/tauri.conf.json`.

## 2026-11-24: landing on Home after signing in, or the app's choice

- **What changed**: signing in, signing up, starting as a guest and
  switching account land on `pageAfterSignInUrl()`, in the seam
  `svelte/src/lib/pages.js`, which returns Home; they landed on Posts.
- **What an app does**: **Optional**: return another page from
  `pageAfterSignInUrl()`, for everyone or person by person.

## 2026-11-24: Avatar, the initials when a picture doesn't load

- **What changed**: the core component `Avatar` draws a person's
  picture, else their initials, else a person icon, and the initials
  when the picture doesn't load, such as a dev server's `localhost`
  address seen from a phone, or a deleted file. `AccountMenu` and
  `ProfileEditPage` use it. `.avatar-initials` no longer sets a size;
  `.avatar-sm` (28px) and `.avatar-lg` (5rem) do.
- **What an app does**:
  - **Optional**: `<Avatar user={...} />` wherever the app shows a
    person's picture, in place of an `<img>` of `avatar_url`.
  - **Required**, for an app that used `.avatar-initials` itself: add
    `.avatar-sm` beside it for its old size.

## 2026-11-24: the app's name in the identity seam

1f7ddaa.

- **What changed**: `APP_NAME` moved from the framework's
  `core/src/services/mailer/mails.rs` to the seam
  `core/src/app/identity.rs`, beside `APP_IDENTIFIER`. It names the app
  in its emails and is the server's name when `SERVER_DISPLAY_NAME` is
  unset. A test of the Tauri app keeps it equal to the `productName`
  and the main window's `title` in `tauri/tauri.conf.json`.
- **What an app does**:
  - **Required**: set `APP_NAME` in `core/src/app/identity.rs` to the
    app's name, the same as `productName` and the main window's `title`
    in `tauri/tauri.conf.json`; `cargo test --workspace` fails until
    the three agree.
  - **Required**, for an app that changed `APP_NAME` in `mails.rs`:
    move its value to `identity.rs` and remove that `[[divergence]]`
    from `framework.toml`.

## 2026-11-24: support, the forum and communities can be turned off

da06b1c, aa0b178, 0b57080, a01cf2e.

- **What changed**: `SERVER_SETTINGS` takes `"support": false` and
  `"forum": false` in `features`, and `"communities": "single"` beside
  `features`, see "Server settings" in `docs/design/multi-server.md`.
  A disabled app has no tab and its paths redirect to Home. With a
  single community the community menu, the Communities tab and the
  community choices on the post and file forms are gone, and a local
  user's communities are merged into the account's when they sign up
  or sign in, through each synced table's
  `SyncedTable::move_rows_to_community`. `svelte/src/lib/pages.js`
  gained `APP_PAGES_REQUIRED_FEATURES`, the feature each app's pages
  need.
- **What an app does**:
  - **Optional**: an app without support, a forum or several
    communities adds those keys to `SERVER_SETTINGS` in `.env.example`
    and `.env`, and in each deployment's environment.
  - **Optional**: an app of its own that a server may turn off gets a
    field in `core/src/app/features.rs` and its path in
    `APP_PAGES_REQUIRED_FEATURES`.
  - **Required**, for an app with synced tables of its own in
    communities: implement `move_rows_to_community` for each, as
    `core/src/app/posts_communities.rs` does. The default moves nothing,
    so on a single-community server a local user's rows would be left
    in a community that is deleted.

## 2026-11-24: wide pages

27d8544.

- **What changed**: `Page` takes `wide`, a wider column, for tables,
  code or long lines.
- **What an app does**: **Optional**: `<Page wide>` on such pages, in
  place of a width of the app's own.

## 2026-11-24: bin/dev on the tailnet

bcd10f1.

- **What changed**: when Tailscale is installed and up, `bin/dev` builds
  the pages and runs `tailscale serve` for the web server, so a phone on
  the tailnet reaches it at the machine's tailnet name, on the web
  server's port.
- **What an app does**: **Optional**: `TAILSCALE_SERVE=false` in `.env`
  keeps the web server off the tailnet.

## 2026-11-23: the app's own pages, services, server tasks and state

a10c3c2, 7fe89a5, 0e3f24f, a904116, 0ae5f29, 8fe6257.

- **What changed**: new seams for an app's own parts:
  `svelte/src/lib/pages.js` (its pages, out of `router.js`),
  `core/src/services.rs` (its clients of other services),
  `core/src/app/server_tasks.rs` (tasks the server runs while it is up,
  each a `ServerTask`) and `core/src/app/server_state.rs` (what the
  server keeps between requests, `ServerState`, which `ServerTask::run`
  receives too). A route that must not answer a page merely open in a
  signed-in browser takes `CurrentUserFromBearerToken`. `bin/dev` tells
  the web server where the Svelte dev server is, `PAGES_DEV_URL`.
- **What an app does**:
  - **Required**, for an app that added pages to `router.js`: list them
    in `APP_PAGES` in `pages.js` and take `router.js` back to the
    template's.
  - **Required**, for an app with a `ServerTask`: give its `run` the
    `state: &Arc<ServerState>` argument.
  - **Optional**: keep a cache or a background loop of the app's in
    `ServerState` and a `ServerTask`.

## 2026-11-23: the header scrolls away on small windows

5927fa3.

- **What changed**: below 40rem the app's header is the top of the page
  and scrolls away with it; on wider windows it stays fixed.
- **What an app does**: **Required**, for an app with sticky elements:
  stick them below the header on wide windows,
  `top: calc(var(--app-header-inset-top) + var(--app-header-height))`,
  and at the window's top below 40rem.

## 2026-11-23: sign-in by password alone, nothing sold

11782b4, 29e12e5.

- **What changed**: `SERVER_SETTINGS` takes
  `"accounts_sign_in_code": false` (sign in by password alone) and
  `"accounts_purchasing": false` (nothing sold); the server's command
  line makes, resets and deletes accounts (`accounts users`); `bin/dev`
  reads `SERVER_SETTINGS` from `.env`.
- **What an app does**: **Optional**: an app whose accounts are made by
  hand, or that sells nothing, sets those keys in `.env.example`.

## 2026-11-23: every page through ttag, checked by bin/lint

bc4c8df, 1aa2a1d, 37e6fb0, ea5f927, 191f655.

- **What changed**: every word a page shows goes through ttag, and
  `pnpm translations:check`, run by `bin/lint`, fails on English that
  skips it; a person chooses their language; emails are sent in it;
  times and money are shown in it, through `svelte/src/lib/formats.js`;
  an app lists its own languages in `core/src/app/locales.rs`.
- **What an app does**: **Required**: write the app's pages' words
  through `t`, `ngettext` or `jt`, show dates and amounts through
  `formats.js`, and run `bin/lint` before a commit.

(2026-11-22's changes, server settings, signing in to another server
and the sign-in pages, are not written up here; see `git log`.)

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
