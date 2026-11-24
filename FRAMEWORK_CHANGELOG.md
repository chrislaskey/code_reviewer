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
