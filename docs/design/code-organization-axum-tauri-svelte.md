# Code organization in axum, tauri and svelte

Status: built 2026-11-21, steps 1 to 4 below.

## Request

Core is organized as the project owner's Elixir applications are
(`code-organization.md`). Apply the same care to the layers over it:

- **The Svelte pages**: one dedicated page per URL, RESTful paths and
  file names (`index`, `show`, `new`, `edit`), reusable components in
  the core components, and a page's own larger pieces in a
  `components/` folder beside it.
- **The Axum server and the Tauri app**: both hand the pages' calls to
  core, each over its own transport. What they share should have the
  same file names in the same folders; what only one has (the server's
  configuration and command line, the device's accounts, sync and
  native macOS and iOS code) should have a clear place of its own.

## The rule: the middle layers mirror core

A handler in `axum` and a command in `tauri` sit at the path of the
core module they call, so one resource is found at the same path in
all three crates:

| core | axum (routes) | tauri (commands) |
|---|---|---|
| `app/posts.rs` | `app/posts.rs`, `posts::list` | `app/posts.rs`, `posts_list` |
| `accounts.rs` | `accounts.rs`, sign-up and sign-in | `accounts.rs`, the browser sign-in |
| `accounts/products.rs` | `accounts/products.rs` | `accounts/products.rs` |
| `accounts/trial_users.rs` | `accounts/trial_users.rs` | (web only) |
| `ops/staff.rs` | `ops/current_staff.rs` | (web only) |

A resource the pages call but core has no module of that name for
(`current_user`, `checkouts` on the device) takes the API's name. A
handler file holds only handlers; a command file only commands and
what they alone need.

Everything else in a crate is that crate's own machinery and sits at
its root, as `database.rs` and `deployment.rs` sit at core's.

### axum

```
axum/src/
  main.rs           start: the server, or a command line
  router.rs         every route, grouped by app
  state.rs          AppState, what a handler may take
  api_error.rs      a core error as the API answers it
  user_auth.rs      CurrentUser, from the token or the cookie;
                    CurrentUserFromBearerToken, the token alone;
                    CurrentUserFromCookie, the cookie alone
  rate_limits.rs    the accounts routes' limits
  config.rs         host, port, proxy, trial cap; config/: database,
  config/           mailer, storage, stripe, app_store, server_url
  jobs.rs           the recurring jobs' loop
  command_line.rs   `app_web <command>`; command_line/: jobs, ops_staff,
  command_line/     accounts_products
  public_pages.rs   the public website's pages
  sync.rs, sync/    sync's routes: push, pull, backfill, and the bytes of synced rows
  app.rs, app/      the app's routes, one module per resource
  accounts.rs, accounts/   the accounts app's
  ops.rs, ops/      the ops app's: the staff gate, its API and pages
  tests.rs, tests/  the router's tests, one file per app
```

### tauri

```
tauri/src/
  main.rs, lib.rs   start: the builder, setup, the command list
  command_error.rs  FormError, a command's error as a form shows it
  user_auth.rs      signed_in, the active account's user
  device_accounts.rs  the accounts on this device and their files
  sync.rs, sync/    the sync runner and its commands, with conflicts and
                    pending changes
  server_urls.rs    where the web server is
  public_pages.rs   the public pages, opened in the browser
  app.rs, app/      the app's commands, one module per resource
  accounts.rs, accounts/   the accounts app's: sign-in and its
                    desktop loopback and iPhone sheet, the phone's
                    store sheet, products, checkouts, purchases,
                    subscriptions, store_transactions, deletions
  quick_picker/     the macOS quick picker window
  clipboard/, macos/  native calls Tauri has no API for
```

Native code lives beside the feature that needs it when only that
feature does (the loopback, the sign-in sheet, the store sheet, all
under `accounts/`), and in a folder of its own when several do
(`clipboard/`, `macos/`). The Swift plugins stay in `tauri/plugins/`.

## Svelte

```
svelte/src/
  index.html, main.js       the app's entry; ops.html, quick-picker.html
  lib/
    components/   core components: the atoms, and the generic pieces
                  every page uses: Page, Dialog, DialogActions,
                  FieldErrors, TileGrid
    layouts/      AppLayout and what only it shows: the header, nav,
                  bottom bar, menus and banners
    core/         the calls into app_core
    *.svelte      pieces that know the app, shared across folders:
                  FileImage, AccountsNeeded, PublicPagesLinks, ...
    *.js          routes, router, current_user, ...
  home/           HomePage
  posts/          PostsIndexPage, PostsShowPage, PostsNewPage,
                  PostsEditPage; components/PostsForm
  files/, communities/, conflicts/, profile/, support/, forum/
  accounts/       the accounts app; one folder per resource under it,
                  as its paths are: products/, checkouts/, purchases/,
                  subscriptions/, deletions/, password/
  ops/, quick-picker/   entries of their own
  public/         the public website, plain HTML
```

- **A folder per path.** A page's folder is its path's: `/posts/:id`
  is `posts/PostsShowPage.svelte`, `/accounts/products/:id` is
  `accounts/products/AccountsProductsShowPage.svelte`. The file keeps
  its whole name, resource then action then `Page`.
- **RESTful paths.** A collection's pages are `index`, `show`, `new`
  and `edit`; a singular resource has no id, so the person's profile is
  `/profile/edit`, their password `/accounts/current/password/edit`, and their
  account, the signed-in one, `/accounts/current`. The
  sign-in page keeps the name Phoenix's own `gen.auth` gives it,
  `/accounts/sign-in`, signing up too, as
  do `/accounts/trial` and `/accounts/back-to-app`, which are no
  resource's.
- **`components/` beside the pages** holds a folder's pieces that more
  than one of its pages use (a form shared by new and edit) or that
  are large enough to stand alone (a dialog). A piece used in one
  place and small stays in its page.
- **A piece used across folders** moves to `lib/`: a generic one to
  `lib/components/`, exported from its `index.js`; one that knows the
  app to `lib/` itself.

## API paths

The API is RESTful by resource, as the pages are. A resource's upload
URL is made by `POST .../uploads`: `/api/users/current/avatar/uploads` and
`/api/files/uploads`.

## Steps

1. Svelte: `lib/components` and `lib/layouts`; page folders with
   `components/`; the accounts subfolders and `home/`; the two
   singular paths, `/profile/edit` and `/accounts/current/password/edit`.
2. Axum: `state`, `user_auth`, `config`, `command_line`; handlers into
   `app/`, `accounts/`, `ops/`; `/api/files/uploads`; the router's
   tests split by app.
3. Tauri: `command_error`, `user_auth`; commands into `app/` and
   `accounts/`, `purchasing.rs` split by resource.
4. Update `AGENTS.md`, `README.md`, `multi-app.md` and the other
   designs' paths.

## As built

- `axum`'s CurrentUser extractor and the browser's token cookie are
  `user_auth`; the `GET`/`PUT /api/users/current` handlers are
  `app::current_user`. "Try it now" is `accounts::trial_users`, as in
  core. The `StaffError` answer joined the others in `api_error`.
- `tauri`'s `FormError` and every `From` for it are `command_error`, as
  the API's are `api_error`. Purchasing is one module per resource:
  `accounts::products`, `checkouts`, `purchases`, `subscriptions` (the
  store's own subscriptions page included), with `plans::refresh` and
  `stripe_pages` between them.
- The router's tests are `axum/src/tests.rs`, the shared helpers, and
  `tests/app.rs`, `accounts.rs`, `ops.rs` and `router.rs`.
- In Svelte, `file_size.js` is in `lib/`, used by the files pages and the
  quick picker; `AccountsNeeded` too, used by the accounts, support and
  forum pages. `FilesNewPage` (400 lines) and `AccountsIndexPage` (270)
  are the largest pages; each already has its pieces in `components/`.
