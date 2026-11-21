# Multi-app

One deployment carries the whole product, not only the application:
the public website, account management, support, a forum, and the
staff's tools. Each is an **app**: a boundary inside the one web
server, the one database and the one sign-in, not a service of its own.

Status: the structure is built, 2026-11-10: the tables' names, the two
rows of a user, who is staff, the paths, the nav and a placeholder
landing page for `accounts`, `support`, `forum` and `ops`. `public` is
built, see `public-pages.md`. Each other app's own tables, API and
pages are planned one at a time; what is still to decide is at the end.

## The apps

| App | What | Tables | On a device |
|---|---|---|---|
| `app` | the application itself | `app_*`, synced | all of it, offline |
| `public` | the public website | none | never |
| `accounts` | sign-in, the account, purchasing | `accounts_*` | online only; a phone buys with the store's in-app purchase |
| `support` | support requests | `support_*` | online only |
| `forum` | discussion | `forum_*` | online only |
| `ops` | the staff's dashboard | `ops_*` | never |
| `sales` | deferred | | |

The forum is `forum`, not "community": a community is already the
boundary inside a deployment.

## An app is a name in four places

`app` is the default everywhere, so what exists stays where it is. Any
other app carries its name:

| Place | `app` | Another app |
|---|---|---|
| Table | `app_posts` | `support_tickets` |
| Core module | `core/src/app/posts.rs` | `core/src/support/` |
| API path | `/api/posts` | `/api/support/...` |
| Page path and folder | `/posts`, `svelte/src/posts/` | `/support/...`, `svelte/src/support/` |

- **Rust modules**: `core/src/app.rs` declares the app's high-level
  contexts under `core/src/app/`, alongside `accounts.rs`/`accounts/` and
  `ops.rs`/`ops/`, called by their paths, `app_core::app::posts`. Each
  table's records are `core/src/data/<table>/`, named after the table,
  see `docs/design/code-organization.md`.
- **Paths**: `app` owns the top level and is the fallback; another app
  owns its subpath. The public website owns `/` and its pages' names.
- **Pages**: one Svelte app, each page loaded when visited, as today.
  `ops` is the exception: a build of its own, `vite build --mode ops`
  of `svelte/src/ops.html` and `svelte/src/ops/` to `svelte/dist-ops`,
  its files under `/ops/`. The staff's tools are not in what users
  load, and not in the desktop and phone apps, which bundle
  `svelte/dist`. A link to it loads the page in full.
- **The nav** (`svelte/src/lib/AppNav.svelte`) links the apps: Support,
  Forum, the person's email to their account, and Ops for staff.
- **Depends on**: `accounts`, then `app`, then every other app. Never
  the other way.

## Only `app_*` syncs

The prefix says which app a table is, and that says whether it leaves
the server: `app_*` tables sync to devices, every other table is the
server's alone.

- The sync's own tables are the app's: `app_events`, `app_jobs`,
  `app_purges`, `app_sync_state`, `app_sync_backfills`.
- `device_*` tables are a device's own and never synced.
- On a device, `accounts`, `support` and `forum` are online only: their
  pages call the web server's API with the device's token, and a change
  is the server's at once. Nothing of theirs is in the device's
  database.
- Purchasing on a phone is the store's in-app purchase. What a device
  must know offline, the person's plan, it keeps from the last time it
  was online, in `device_settings`, never through `app_users`, which
  other members see.

## A user is two rows

One id, two tables, written together at sign-up:

| Table | Holds | Leaves the server |
|---|---|---|
| `accounts_users` | email, `confirmed_at`, later the subscription | never |
| `app_users` | name, `avatar_url`, the sync columns | synced, to the members of the user's communities |

- An email never reaches other people's devices. A page shows a user
  by **name**, asked for at sign-up (`accounts::sign_up`), which writes
  both rows in one transaction. A person changes their name on their
  profile, `users::update`: an event on their own row, so a device
  writes it offline and syncs it, and nobody changes another's.
- `accounts_tokens` and `accounts_credentials` reference
  `accounts_users`. A device has the `accounts_` tables and leaves them
  empty.
- A person is shown their own email by `GET /api/current-user`, which
  adds it and `is_staff` to their user; a device adds it from its list
  of accounts, which a sign-in's answer fills.
- The purge removes an unconfirmed sign-up's two rows after a day, and
  the account of any user it removes.
- A new user starts with a community of their own, `Personal`, made
  when their email is confirmed, so an app may ask for a current
  community everywhere.
- A person may use the app before signing up: a local user on a
  device, an `app_users` row with no account anywhere, or a trial user
  on the web, an account with `trial_expires_at`. The pages read which
  from the current user's `kind`, see
  `accounts-local-and-trial-users.md`.
- `forum` and `support` show the same name and picture, from
  `app_users`, and read `accounts_users` to email someone.

## Staff

`ops_staff` has a row per account that may use `ops`
(`core/src/ops/staff.rs`).

- To anyone who is not staff, signed in or not, every path of the ops
  app is a 404: the API under `/api/ops`, answered as an API path with
  no route is, and the pages and their files under `/ops`
  (`axum/src/ops.rs`). A 403 would say there is something there.
- Someone becomes staff from the server's command line, never from a
  page: `app_web ops staff create <email>`, and `ops staff delete
  <email>`.
- In dev the Svelte dev server gives the ops page to anyone; its API is
  the web server's, so it shows nothing.

## Still to decide

- The build order of the apps. Proposed: `ops` with a users list;
  `support`; purchasing in `accounts`; `forum`.
- ~~How a device's pages call an online-only app's API~~: decided
  2026-11-11 by purchasing, see `app_core::accounts::online`. A Tauri
  command per call asks the server's API with the device's token, held
  in Rust, through the sync's `Http`, and passes its JSON on; the page
  calls it from `svelte/src/lib/core/tauri-commands.js` as any other.
- Whether a deployment can leave an app out, so `ops` can run where
  only staff reach it.
