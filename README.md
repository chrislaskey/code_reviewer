# Starter Pack

One application in Rust, with Svelte pages, deployed three ways:

* **web**: `axum/`, a web server with the application behind a JSON API,
  the public website's pages (`svelte/src/public/`, plain HTML, see
  `docs/design/public-pages.md`) and the Svelte pages at every other path
* **desktop**: `tauri/`, a Tauri app with the application behind Tauri
  commands and the Svelte pages in its window
* **mobile**: the same Tauri app, built for iOS and Android

```
core/      app_core, the application: one module per resource, with
           list/get/create/update/delete functions returning plain data,
           the database they read and write, and its migrations
axum/      app_web, the web server: app_core over HTTP under /api, and svelte/dist
tauri/     app_tauri, the desktop and mobile app: app_core behind Tauri commands
svelte/    the pages, one Svelte app the web server serves and the Tauri app bundles
bin/dev    starts a deployment in dev mode
bin/reset  deletes the local SQLite files and drops the dev PostgreSQL database
bin/storage runs Garage, the S3-style bucket uploads go to in dev
phoenix/   the Phoenix version this one replaces in stages, kept as the reference
```

Rust is here for the one language running on the web, the desktop and
phones, not for speed; `AGENTS.md` says how the code is written.

## How a page reaches the application

A page calls `postsList()`, `postsGet(id)`, `postsCreate(body)`,
`postsUpdate(id, body)` or `postsDelete(id)` from `svelte/src/lib/core/`.
Inside the Tauri app each is a command of the same name
(`tauri/src/posts.rs`); in a browser each is a request to the JSON API
(`axum/src/posts.rs`):

| Call | Request |
| --- | --- |
| `postsList()` | `GET /api/posts`, each post with its owner and communities |
| `postsGet(id)` | `GET /api/posts/:id` |
| `postsCreate(body, communityIds)` | `POST /api/posts` with `{"body": "...", "community_ids": [...]}`, 201 |
| `postsUpdate(id, body, communityIds)` | `PUT /api/posts/:id` with the same |
| `postsDelete(id)` | `DELETE /api/posts/:id`, 204 |
| `communitiesList()` | `GET /api/communities`, each with the user's membership or null |
| `communitiesGet(id)` | `GET /api/communities/:id`, with the user's membership or null |
| `communitiesCreate(name)` | `POST /api/communities` with `{"name": "..."}`, 201, the maker a member |
| `communitiesUpdate(id, name)` | `PUT /api/communities/:id` with the same; a member only, else 403 |
| `communitiesDelete(id)` | `DELETE /api/communities/:id`, 204; a member only, and every membership goes with it |
| `membershipsJoin(communityId)` | `PUT /api/memberships/:community_id` |
| `membershipsLeave(communityId)` | `DELETE /api/memberships/:community_id`, 204 |
| `filesList()`, `filesGet(id)` | `GET /api/files`, `GET /api/files/:id`, each file with how it is shown and where it stands |
| `filesSha256(file)`, `filesListDuplicates(sha256s)` | the picked file's SHA-256, worked out in the page; `POST /api/files/duplicates` with `{"sha256s": [...]}`, up to 500, the files the user sees with any of them |
| `filesCreate(file, name, communityIds, onProgress?, sha256?)`, `filesCreateAtOnce` | `POST /api/files/upload` for where to PUT the bytes, the PUT to the bucket, then `POST /api/files`, 201; in the app one command, the bytes kept on the device and sent by the next sync. A page creates `filesCreateAtOnce` files at a time, 3 on the web and 1 in the app |
| `filesUpdate(id, name, communityIds)` | `PUT /api/files/:id` with `{"name": "...", "community_ids": [...]}`, its communities among the user's |
| `filesDelete(id)` | `DELETE /api/files/:id`, 204: out of the user's communities, deleted when those were its last |
| `filesContentUrl(id)` | `/api/files/:id/content`, a redirect to the bytes for an `<img>`; in the app the `files` protocol, serving the device's copy |
| `filesDownload(id)`, `filesDownloadSupported()` | the same URL with `?download=1`; in the desktop app a copy into Downloads |
| `filesCopy(id)`, `filesCopySupported()` | the image onto the clipboard: on the web as a PNG, in the macOS and iOS apps with its own bytes too |
| `filesCacheGet()`, `filesCacheDelete()` | the app only: the copies of synced files kept on the device, and removing them |
| `conflictsList()`, `conflictsReapply(id)`, `conflictsTryAgain(id)`, `conflictsLetGo(id)` | none: the web has nothing to resolve |
| `currentUserGet()` | `GET /api/current-user`; null when nobody is signed in, which the server answers 401 |
| `currentUserUpdate(name)` | `PUT /api/current-user`, their new name; in the app written on the device and synced |
| `currentCommunityGet()` | `GET /api/current-community`, the community the person works in, or null when they are in none |
| `currentCommunityUpdate(communityId)` | `PUT /api/current-community` with `{"community_id": "..."}`; a member only, else 403 |
| `avatarsCreateUpload(contentType, size)` | `POST /api/avatars/uploads`, 201 with where to PUT the picture; 422 with `"errors"` for a wrong file |
| `avatarsUpdate(key)` | `PUT /api/avatars` with `{"key": "..."}`, the user with their `avatar_url` |
| `avatarsUpload(file, onProgress?)` | both, with the file PUT straight to the bucket between them, see [Forms and uploads](#forms-and-uploads) |
| `accountsSignUp(email, name)` | `POST /api/accounts/sign-up`, 202 with the pending token, a code emailed |
| `accountsSignIn(email)` | `POST /api/accounts/sign-in`, the same |
| `accountsCode(pending, code, challenge?)` | `POST /api/accounts/code`, the user and the token, and the cookie; with a device's challenge, the device code and no cookie; or `{next, pending}` when a password comes first |
| `accountsSignInPassword(pending, password, challenge?)` | `POST /api/accounts/sign-in/password`, the account's password after the code; answers as a finished code |
| `accountsSignInResetPassword(pending, password, challenge?)` | `POST /api/accounts/sign-in/reset-password`, a new password in place of a forgotten one; answers as a finished code, 403 when the app locks out |
| `accountsSignInNewPassword(pending, password, challenge?)` | `POST /api/accounts/sign-in/new-password`, the password a new account makes, or null to skip; answers as a finished code |
| `accountsSecondFactors()` | `GET /api/accounts/second-factors`, `["password"]` or `[]` |
| `accountsPasswordUpdate(current, password)` | `PUT /api/accounts/password`, 204; the current one when there is one |
| `accountsCodeResend(pending)` | `POST /api/accounts/code/resend`, 202, a new code emailed |
| `accountsSignOut()` | `DELETE /api/accounts/sign-out`, 204 |
| `accountsSignInStart(email?)`, `accountsSignInPending()`, `accountsSignInFinish(code)`, `accountsSignInCancel()` | none: the web signs in on its own page; the app's `POST /api/accounts/device-token` is what `accountsSignInFinish` calls, see "Accounts on a device" |
| `pendingChangesList()` | none: the web has nothing waiting |
| `deviceAccountsList()`, `deviceAccountsSwitch(userId)`, `deviceAccountsDeactivate()`, `deviceAccountsRemove(userId, losingChanges)` | none: the web keeps no accounts, see "Accounts on a device" |

Both call the function in `app_core::posts` (`core/src/app/posts.rs`),
`posts::list`, `posts::get` and so on, named through their module as the
standard library's `fs::read` is, as the user acting, see [Who the
user is](#who-the-user-is), and answer with the same JSON, serialized
from the same Rust types, so the page never knows which it is on. The
rules live there too: a post's body is trimmed and may not be blank, and
only its owner edits or deletes it. The server answers an error as a
status, 404 for a missing post, 403 for someone else's and 422 for a
blank one, and `{"error": "..."}`; the app answers the message; and the
page shows the message either way.

To add to a resource, or add one: the function in `core/`, a route in
`axum/src/router.rs` with its handler, a command in `tauri/src/lib.rs`
with its function, named with the resource (`posts_list`, its name on
the wire too), the two calls in `svelte/src/lib/core/`, then the page.

How the apps' local databases and the web server, the system of record,
converge is under "Sync".

## Who the user is

Every core function takes the `users::User` acting (`core/src/app/users.rs`):
what they write is theirs, `user_id` on a post, and what they read is
what they own or what is in a community of theirs, see
[Communities](#communities). `app_users` is the app's view of each person, one row
with their name and picture, written by the server at sign-up and
synced to devices like any row. Their email, with `confirmed_at` once
they proved it, is their account's, `accounts_users`
(`core/src/accounts/users.rs`), a row with the same id that stays on the
server, so an email never reaches another person's device; see
`docs/design/multi-app.md` for the apps one deployment carries and
which tables are whose. Credentials never leave the server either: the password hash
is in `accounts_credentials` (`core/src/credentials.rs`, Argon2id) and
every secret the server hands out in `accounts_tokens`
(`core/src/tokens.rs`), kept as hashes, each in a context that sets how
long it lives: `api` for the apps' bearer tokens, a year; `session` for
the browser's cookie, sixty days; `sign_in_code` for a sign-in waiting
for its emailed code, ten minutes; `device_code` for a finished browser
sign-in a device collects, five minutes.

Signing up, in and out is `core/src/accounts.rs`, on the server, and
every sign-up and sign-in ends with a six-digit code emailed to the
person, the design in `docs/design/accounts-sign-in.md`. A sign-up
writes the account, unconfirmed, and the user with their name, a sign-in finds
the account by its email; both then make a pending sign-in, email its code
(`core/src/mails.rs` through `core/src/mailer.rs`) and answer the
pending token. The code, five wrong tries at most, confirms the email
the first time. A password is optional, a second factor: an account
that has one is asked for it after the code, a new account is offered
to make one or skip, and a signed-in person makes or changes theirs at
`/accounts/password`. A forgotten one is reset at the password step,
unless `accounts::FORGOTTEN_PASSWORD` is changed to lock out. The last step makes the token; a sign-out deletes it. Emails are
lowercased, and a sign-up with an email that has an account is answered
as a new one, its owner emailed instead. Where the email goes is
`MAILER`: unset or `log`, the server logs it in full, which is how a dev
reads the code; `resend`, it goes through [Resend](https://resend.com)
with `RESEND_API_KEY` and `MAIL_FROM` (`axum/src/mailer.rs`). The
server resolves the user of every other request from its token,
`axum/src/current_user.rs`: the apps send it as `Authorization: Bearer`,
and a browser sends the cookie the code set (`HttpOnly`,
`SameSite=Strict`, `Secure` away from localhost). A request with no
token, or a token the server does not know, is 401, whatever it asks
for; the pages send a visitor to the sign-in page before showing
anything that needs one (`svelte/src/lib/router.js`). The accounts
routes are rate limited per address, per pending sign-in and per
client IP (`axum/src/rate_limits.rs`, the `governor` crate); behind a
reverse proxy, `TRUST_PROXY_HEADERS=1` takes the client IP from the
last entry of `X-Forwarded-For`, the one the proxy wrote, so the proxy
must be the only way in and must add the address it saw. The proxy
must also let a request be 5 MB, what a device's push may be
(`sync::MAX_PUSH_BYTES`); nginx allows 1 MB unless told.

A person can use the app before signing up
(`docs/design/accounts-local-and-trial-users.md`). In the desktop and
phone apps, Skip on the accounts page makes a local user, on that
device only, who never syncs; signing up or in later offers to move
what they made into the account. On the web, with `TRIAL_USERS_MAX`
set, "Try it now" makes a trial user for 24 hours, deleted with
everything of theirs 48 hours after it was made unless they sign up,
which keeps the same user. The pages tell the kinds apart by the
current user's `kind`: `full`, `local` or `trial`.

Trials are off on a server until `TRIAL_USERS_MAX` is set, since the
number is how much a stranger with no email can make the database
hold: 100 is the number to start with, and what `bin/dev` sets. A
trial user is one of at most that many, makes 500 writes and no more
(`trial_users::MAX_WRITES`), sees only their own communities, has no
files, and a client IP starts ten an hour. A trial refused because
the server holds all it may is a 503, and so an error in the log.

A device asks for no password at a sign-in: it signs in in the system browser,
on the web's own sign-in page, and exchanges the code the page shows
for its token, see "Accounts on a device". It keeps who signed in and
their token as the `signed_in` device setting. A sign-out syncs first,
then tells the server to forget the token and clears the setting; the
data stays, pending changes included, so signing in again resumes from
the cursor. Nobody acts until someone signs in: there is no default user
anywhere. To try a second user over HTTP, with the code read from the
server's log:

```sh
curl -H "Content-Type: application/json" \
  -d '{"email":"alice@example.com","name":"Alice"}' \
  localhost:4000/api/accounts/sign-up   # answers {"pending": ...}; the code is in the log
curl -H "Content-Type: application/json" \
  -d '{"pending":"<pending>","code":"<code>"}' \
  localhost:4000/api/accounts/code      # answers {"next": "new_password", "pending": ...}
curl -H "Content-Type: application/json" \
  -d '{"pending":"<pending>","password":null}' \
  localhost:4000/api/accounts/sign-in/new-password   # skips it; answers her token
curl -H "Authorization: Bearer <token>" localhost:4000/api/posts
```

What was left out, changing the email and the rest of the Phoenix flow, is
`docs/suggestions/accounts.md`.

## Forms and uploads

A form's rules are declared on its request's struct in core with
`garde`, `#[garde(custom(is_image_type))]`, and checked with
`FieldErrors::check` (`core/src/field_errors.rs`), which answers the
messages by field, as an Ecto changeset's errors. The server answers
them with 422 as `{"error": "...", "errors": {"field": ["message"]}}`,
an app's command answers the same object, and both are thrown to the
page as a `FormError` (`svelte/src/lib/core/form_error.js`), whose
`errors` the `FieldErrors` component shows under the input they belong
to. The page checks nothing itself beyond `accept` and `required`.

A file goes from the person's device straight to object storage, never
through the server (`core/src/app/avatars.rs`, `core/src/storage.rs`, over
`object_store`): the server checks what was picked and answers a URL
signed for its key and content type, the page PUTs the file there, and
the server checks the file arrived and is the image it said before the
user's `avatar_url` points at it. The Profile page sets the picture.
This pattern is online only, see [Files added offline](#files-added-offline)
for the other; the design, the failure cases and how
files are deleted are `docs/design/uploads.md`. In dev the bucket is
Garage, which `bin/dev` starts with the web server when it is installed
(`brew install garage`); without it the server runs with uploads off.

## Files added offline

A file (`core/src/app/files.rs`) is the second upload pattern: a resource
of its own whose row syncs like a post while its bytes travel apart. A
file belongs to its communities, not to its creator: a live member of
one sees it, and it lives as long as it is in one
(`core/src/app/files_communities.rs`, `docs/design/files-communities.md`). In
the app a file picked on the New page is kept at once in the account's
folder on the device, `accounts/<user id>/files/<file id>`, with a row
whose create waits in the queue, so it is listed and shown with the
network off. A sync asks the server which waiting files' bytes its
bucket has, pushes everything else, pulls, then PUTs the bytes to a
signed URL and pushes the files' events; the server writes a file's row
only once its bytes are in the bucket. Another device gets the row by
the pull and the bytes the first time a page shows the file, through
the app's `files` protocol (`tauri/src/files.rs`), and keeps them. On
the web the page PUTs the bytes first, then creates the row.

A file is waiting, refused or synced. Waiting is tried by every sync;
the server saying no 30 times, or a file it will not take at all,
makes it refused, kept on its device and listed on the conflicts page
to try again or let go. Bytes follow rows on both sides: the server's
sweep (`files::delete_unreferenced`) and a cleanup of the account's
folder at the end of each sync. The Files page shows images, animated
ones too unless "Play animations" is off, plain text and PDFs in
place. The decisions are `docs/design/offline-files-decisions.md`, how
they were reached `docs/design/offline-files.md`, and what was left
out `docs/suggestions/files.md`.

## Communities

A community (`core/src/app/communities.rs`) is the soft boundary inside a
deployment: any user creates one and joins it, every user sees every one
and may join or leave (`core/src/app/memberships.rs`), and a post is in the
communities its owner chose, zero or more
(`core/src/app/posts_communities.rs`), which is what the New and Edit pages'
checkboxes set. A user sees their own posts and the posts in their
communities; leaving a community takes the member's own posts there
along, and rejoining brings them back. The decisions are
`phoenix/docs/design/multi-tenancy.md`, the port is in
`docs/design/offline-sync.md` under "Communities and what a user sees",
and what was left out is `docs/suggestions/communities.md`.

Application resource workflows live under `core/src/app/`, declared by
`core/src/app.rs`, alongside the accounts and ops modules. They remain
re-exported at the crate root, so `app_core::posts` still works alongside
`app_core::app::posts`. Database/deployment code, shared helpers and
`data/`, `sqlite/`, `postgres/` remain at the root.

## The database

Every deployment opens a database of its own and runs the same migrations
on it, so all three have the same tables: the web server's is the system
of record the apps will sync with; the desktop and mobile apps each keep a
SQLite file on the device, so they work with the network off. That is
`core/src/database.rs`: a `Database` enum of the open connection pool,
SQLite, or PostgreSQL for a web server that has outgrown a file. Public
application operations take a `Deployment` (`core/src/deployment.rs`), the
database with its role, `Server` for the web server and `Device` for the
apps, and own one `Transaction`, so a row and its event commit together.
Internal record contexts under `core/src/data/` borrow that transaction
and dispatch to private SQLite/PostgreSQL adapters (for example,
`core/src/data/posts/sqlite.rs` and `postgres.rs`). Workflows compose
contexts for authorization, sync and multi-resource changes. Atomic bulk
lifecycle and maintenance operations are explicit exceptions documented
in `docs/design/low-level-contexts.md`. The backend root modules contain
only migration runners; there is no shared SQL-dispatch bypass macro.

The database is [sqlx](https://github.com/launchbadge/sqlx): async, one
library for both databases, with `$1` placeholders in both. Ids are
UUIDv7, made by whichever device writes the row, so an offline write never
collides; they are text in SQLite, so the file reads well in the `sqlite3`
shell. Times are `DateTime<Utc>` to the microsecond, RFC 3339 text in SQLite
and `timestamptz` in PostgreSQL. The migrations are `.sql` files under
`core/migrations/sqlite/` and `core/migrations/postgres/`, one per
database since their SQL differs, named `<version>_<name>.sql` with the
name resource first (`posts_create`), embedded
at build time by sqlx's `migrate!` and run when a database opens; add
one by hand or with the sqlx CLI (`cargo install sqlx-cli`, then
`sqlx migrate add --source core/migrations/sqlite <name>`). `core/`'s
`sqlite` feature is on by default and the web server turns on `postgres`
too; the Tauri app only ever has SQLite, so no PostgreSQL client is
compiled into the desktop or phone build.

Where each deployment's file is:

* The web server: `DATABASE_URL`, `sqlite:<path>` relative to `axum/`,
  by default `sqlite:data/app.db`, a gitignored file in the worktree, or
  `postgres://user@host/database`, which must exist (`createdb`). So
  `DATABASE_URL=postgres://postgres@localhost/starter_pack bin/dev` runs
  the web on PostgreSQL while the apps keep their SQLite files, the
  mix the sync will face.
* The Tauri app: a folder per account under `accounts/`, holding its
  database, `app.db`, and the bytes of its files, `files/`, listed in
  `device.db`, in `DATA_DIR` when set, else the app data directory Tauri
  gives it, under `~/Library/Application Support` on macOS and inside the
  app's sandbox on a phone. Set `DATA_DIR` in dev to keep worktrees
  apart. See "Accounts on a device".

`bin/reset` deletes every one of these SQLite files and drops the dev
PostgreSQL database, creating it again empty, with the servers stopped,
so the next start rebuilds each from nothing; `--dry-run` lists what it
would do.

Every resource test in `core/` runs through `with_each_database`
(`core/src/database.rs`): against an in-memory SQLite database always,
and against PostgreSQL too when `TEST_POSTGRES_URL` names a database to
use, each test in a schema of its own that is dropped afterwards. That is
what keeps the two databases' modules in step: a function added to one
and not the other fails the tests wherever PostgreSQL is reachable.

```sh
createdb starter_pack_test
TEST_POSTGRES_URL=postgres://postgres@localhost/starter_pack_test cargo test -p app_core --features postgres
```

## Sync

The web server is the system of record. The desktop and mobile apps work
on their own SQLite file, with the network off, and sync with it: the
design, carried over from the Phoenix version, is
`docs/design/offline-sync.md`. In short:

* Every write, on every deployment, is an **event**
  (`core/src/app/events.rs`): the fields the person changed, from the row's
  version they started at, and who made it. The server applies it at
  once, increments the row's `version` and records the event in
  `app_events`; a device applies it at once too and queues it there for
  the next push.
* A **sync** on a device (`core/src/app/sync.rs`, `sync::run`) pushes the
  pending events to `POST /api/sync/push` with the device's token, where
  the server applies each as that user with the same function, refusing
  a change to someone else's post, and answers it; then pulls the rows
  that user sees which the server wrote since the device's cursor from
  `POST /api/sync/pull`, each after the rows it points at; then asks
  `POST /api/sync/backfill` for everything in each community the user
  just joined; then prunes the posts the user no longer sees. A row from
  the server never replaces a row with an event still pending.
* **Recurring jobs** (`core/src/jobs.rs`, `docs/design/jobs.md`): the
  purge, then the sweeps of both buckets, daily on the server. The
  server runs those due every 15 minutes; `cargo run -p app_web -- jobs
  run` does the same once, for a scheduler outside it, and `jobs run
  purge` runs one job now. Each run is recorded in `app_jobs`.
* The **purge** (`core/src/app/purge.rs`, a daily job, see below)
  hard-deletes what was deleted more than 90 days
  ago and records the horizon; a device away longer than that is told to
  start over on its next pull, and does, keeping its pending changes.
* Conflicts are decided by versions, never clocks, field by field. A
  field someone else changed first keeps their value, and the device's
  value is kept as a **conflict** (`core/src/app/conflicts.rs`) the person
  reapplies or lets go on the `/conflicts` page, which the sync status in
  the nav links to. Deletes are soft, `deleted_at`, so they sync like any
  edit.

The app (`tauri/src/sync.rs`) syncs when it starts, when it comes to the
foreground, after every write, from the Sync now button in the nav, and
every half minute, unless the
person chose to work offline from the nav, a choice kept in the device's
database (`device_settings`, one JSON value per key for what a person
chooses on a device, every one declared with its first value in
`core/src/device_settings.rs` and added to a device when the app starts)
that makes a sync return at once with no request until they go online
again. It syncs against the web
server `tauri/src/server_urls.rs` resolves when the app starts:
`SYNC_SERVER_URL` from the environment, which reaches the desktop app
started from a shell, else `syncServerUrl` under `plugins.starter-pack`
in `tauri/tauri.conf.json`, which is compiled into every build and is
the only way a value reaches a phone build, else `http://localhost:4000`.
`bin/dev` sets it, and the sign-in page's URL beside it, with the
`--config` override for the web server it starts. The web has nothing to sync and shows no sync
status.

The device holds exactly what its user sees. The pull brings each row
after the rows it points at, so the foreign keys hold; joining a
community records it in `app_sync_backfills` and the sync asks the server
for everything in it; leaving, or a post taken out of a community, is
followed by a prune of the posts no longer visible.

The sync is tested in `core/` with device databases of several users and
a server database in one process, on SQLite and PostgreSQL both, through
the `InProcess` transport, which resolves tokens as the web server does;
and once over HTTP against a running web server, as the dev user:

```sh
SYNC_SERVER_URL=http://localhost:4000 TEST_SERVER_DATABASE_URL=sqlite:../axum/data/app.db \
  cargo test -p app_core --features sync-http two_devices -- --ignored
```

## Pages

The Svelte app is one document, `svelte/src/index.html`, and a client-side
router (`svelte/src/lib/router.js`, [sv-router](https://sv-router.dev))
shows the page at the path, swapped in place:

| Path | Shows |
| --- | --- |
| `/` | on the web the public website's home page, `public/index.html`, not the router's; in the apps the router sends it to `/home` |
| `/home` | `HomePage.svelte` |
| `/accounts/sign-up` | `accounts/AccountsSignUpPage.svelte`, on the web: the name and email, then the code, then a password to make or skip |
| `/accounts/settings` | `accounts/AccountsSettingsPage.svelte`, the person's own account: their email and the way to their password |
| `/accounts/password` | `accounts/AccountsPasswordEditPage.svelte`, their first password or a new one; asks the server, so online only in the apps |
| `/support` | `support/SupportIndexPage.svelte`, the support app: a placeholder |
| `/forum` | `forum/ForumIndexPage.svelte`, the forum app: a placeholder |
| `/ops` | not the router's: the ops app, a build of its own (`svelte/dist-ops`) with `ops/OpsHomePage.svelte`, for staff and a 404 to anyone else: a placeholder. `cargo run -p app_web -- ops staff create <email>` makes an account staff |
| `/accounts/sign-in` | `accounts/AccountsSignInPage.svelte`, where a signed-out visitor is sent on the web; with `?challenge=`, a device's sign-in, ending with the code to paste into the app |
| `/posts` | `posts/PostsIndexPage.svelte` |
| `/posts/new` | `posts/PostsNewPage.svelte` |
| `/posts/:id` | `posts/PostsShowPage.svelte`, with Edit and Delete |
| `/posts/:id/edit` | `posts/PostsEditPage.svelte` |
| `/files` | `files/FilesIndexPage.svelte`, each with its state when it is not on the server yet |
| `/files/new` | `files/FilesNewPage.svelte`, one file or up to 5,000 picked or dropped, each named, duplicates decided, created together |
| `/files/:id` | `files/FilesShowPage.svelte`, the file shown in place when it is an image, plain text or a PDF, with Download, Rename and Delete |
| `/files/:id/edit` | `files/FilesEditPage.svelte`, renaming |
| `/communities` | `communities/CommunitiesIndexPage.svelte`, to create, join and leave |
| `/conflicts` | `conflicts/ConflictsIndexPage.svelte`, inside the apps: the changes waiting to reach the server and those it did not keep |
| `/accounts` | `accounts/AccountsIndexPage.svelte`, inside the apps: the accounts on the device, to switch, add and remove |

The New and Edit pages share `posts/PostsForm.svelte`, the body and a
checkbox per community the person is in. A write that worked navigates
to the post, or the list after a delete, through `navigateToUrl` in
`router.js`. The post page shows Edit and Delete to the owner, which it
tells by the current user, `currentUserLoad()`, asked once per visit
and kept in `svelte/src/lib/current_user.js`; the nav shows who is
signed in with Sign out, or Sign in and Sign up.

The nav also shows the current community, the one community the person
works in (`core/src/app/current_community.rs`), and a click opens a dialog
of their communities to switch to, which reloads the page. Any page
asks for it with `currentCommunityLoad()` from
`svelte/src/lib/current_community.js`; none filters by it yet. It is
the chosen one while the person is a member of it, else their first by
name, which is then stored, or null when they are in none. The app
keeps the choice as the device setting `current_community_id` of the
signed-in account; the web in the browser's `current_community_id`
cookie.

### Accounts on a device

The app keeps the accounts signed in on it, each in a SQLite file of
its own, `accounts/<user id>/app.db`, listed in `device.db`, and one of them
active, whose file every command reads and writes (`tauri/src/device_accounts.rs`,
the design in `docs/design/device-accounts.md`). The accounts page,
`/accounts`, lists them: the active one, the others to switch to at once
or, signed out, to sign in to again, each with Remove from device, and
Add account. Adding or signing in closes the active account first and
opens the system browser on the web's sign-in page
(`tauri/src/accounts.rs`, the design in
`docs/design/accounts-sign-in.md`): the app keeps a verifier in
`device.db` and puts its hash in the page's URL as the challenge; the
page, done, shows a code; the person pastes it into the dialog
(`accounts/AccountsSignInDialog.svelte`), and the app exchanges the code
and the verifier for its token, which goes into the file of the user
the server names. The app sees no password at a sign-in, and the browser is not
signed in afterwards. Where the page is comes from `BROWSER_SIGN_IN_URL`, else
`browserSignInUrl` in the config beside `syncServerUrl`, which `bin/dev`
sets to the dev server's, else the sync server's own `/accounts/sign-in`. A sign-out
closes the account and keeps its file; Remove from device deletes it,
after a confirmation, and asks again when a change in it has not
reached the server (`accounts/AccountsRemoveDialog.svelte`). Only the
active account syncs. A sync answered 401, the token expired or deleted
elsewhere, signs the account out on the device, data kept. At most 100
accounts. At start every account's file is brought up to date, orphan
rows and files are swept, and the last active account is opened again.
The web has none of this: one browser session, sign in and sign out.

The paths are real paths, named once in `svelte/src/lib/routes.js`, which
pages link through. Every path is reloadable: the dev server, the web
server and the Tauri app all answer an unknown path with `index.html`,
except under `/api/`, where the web server answers a JSON 404. When the
router cannot tell who is signed in, or a page's code does not load,
the page shows why in a banner above it (`svelte/src/lib/page_load_error.svelte.js`).

### Theme and font

The pages are styled with Tailwind v4 and daisyUI, with all of daisyUI's
themes (`svelte/src/lib/theme.css`), and the `hero-*` icon classes come
from the `heroicons` npm package (`svelte/vendor/heroicons.js`) for the
`Icon` component. The theme sets the font, Plus Jakarta Sans, vendored in
`svelte/src/static/fonts` (OFL licence alongside) and served from `/fonts`,
so it works offline on every deployment.

The theme is `data-theme` on `<html>`. `svelte/src/lib/theme_script.js`
sets it before the first paint and switches it on the `app:set-theme`
events the theme menu (`ThemeMenu.svelte`, `ThemeChoices.svelte`, in the
nav) dispatches, keeping the choice in `localStorage` under `app:theme`;
"System" follows the OS light or dark setting. The script isn't bundled:
`vite.config.js` inlines it at the `<!-- theme-script -->` marker in
`index.html`, in dev and in builds. The pages' own styles
(`svelte/src/styles.css`) take their colours from the theme's variables.

### Quick picker

On macOS ⌃⌘T in any app opens a second window, a small panel under the
text caret, listing the signed-in person's posts and images
(`svelte/src/quick-picker.html`, `tauri/src/quick_picker/`). Choosing
one copies it and pastes it where the person was typing; with nobody
signed in it sends them to the app to sign in. The design, its limits
and how to test it by hand: `docs/design/quick-picker.md`.

### Full bleed

The page runs under the window chrome and keeps its content clear of it.
On the desktop the macOS title bar is an overlay (`titleBarStyle` in
`tauri/tauri.conf.json`): the window controls are drawn over the page, the
page pads its top by the bar's height, and an empty strip there
(`.title-bar-drag` in `index.html`, allowed by the
`core:window:allow-start-dragging` permission) drags the window. On a
phone `viewport-fit=cover` lets the page run under the status bar and the
home indicator, and the page pads by the safe-area insets. `main.js` sets
`data-app` on `<html>` to `desktop`, `mobile` or `web`, which the styles
switch on; a browser gets neither inset.

### Translations

The pages' text is gettext, through [ttag](https://ttag.js.org): a page
writes its English as the message, `t` for one, `ngettext` for a count,
`c("verb").t` where the same English means two things:

```svelte
<script>
  import { t, ngettext, msgid } from "ttag";
</script>
<h1>{t`New post`}</h1>
<p>{ngettext(msgid`${count} comment`, `${count} comments`, count)}</p>
```

The translations are `svelte/translations/`: `default.pot`, every
message, and a `<locale>.po` per language. `src/lib/translations.js`'s
`translationsUse(locale)` loads one, a file of its own in the build;
until it does, and for a message with no translation or a fuzzy one, a
page shows the English. Nothing picks a language yet. Converted so far:
the posts pages. The ops app (staff only) and the public pages (plain
HTML) are left out.

In `svelte/`, with GNU gettext (`brew install gettext`):

```sh
pnpm translations:extract   # the messages into translations/default.pot
pnpm translations:update    # that, then merged into every .po, changed English marked fuzzy
msginit --no-translator -l es -i translations/default.pot -o translations/es.po   # a new language
```

## Running

Prerequisites: Rust and Node with pnpm (`.tool-versions`), and for the
Tauri app the Tauri CLI (`cargo install tauri-cli --version '^2' --locked`)
and Xcode's command line tools.

```sh
bin/dev                # the web: the server on 4000, the pages with hot reload on 1420
bin/dev desktop        # the desktop app, its pages from the dev server on 1420
bin/dev desktop ios    # both apps side by side, on the one dev server
```

Or to run everything at once:

```
bin/dev web desktop ios "iPhone 16"
```

`bin/dev` starts the Svelte dev server once and every target named loads
its pages from it, so the desktop and the iOS app can run together; two
Tauri targets compile in turn, since Cargo locks the workspace's build
directory. It takes its ports from `PORT` when set (the server on `PORT`,
the pages on `PORT + 1000`), so worktrees don't collide. Without `PORT`,
when another program holds 4000 or 1420, it warns and uses 4010 and 5010
instead; `bin/dev --help` has the rest. By hand:

```sh
(cd svelte && pnpm install && pnpm build)   # writes svelte/dist and svelte/dist-ops
cargo run -p app_web                        # http://localhost:4000, serving svelte/dist
(cd tauri && cargo tauri dev)               # the desktop app on the pages' dev server
(cd tauri && cargo tauri build)             # the app bundle, under target/release/bundle
```

The web server reads `PORT`, `DATABASE_URL`, `SVELTE_DIST` (default
`svelte/dist`), `SVELTE_OPS_DIST` (default `svelte/dist-ops`, the ops app's
build, for staff), `MAILER` with `RESEND_API_KEY` and `MAIL_FROM`,
`STORAGE_BUCKET` (public, for pictures) and `STORAGE_PRIVATE_BUCKET`
(for files) with `STORAGE_ENDPOINT`, `STORAGE_REGION`,
`STORAGE_ACCESS_KEY_ID`, `STORAGE_SECRET_ACCESS_KEY` and
`STORAGE_PUBLIC_URL` (`bin/storage env` prints Garage's), and
`TRUST_PROXY_HEADERS`, `TRIAL_USERS_MAX` (how many trial users the
server holds at once; unset or 0, "Try it now" is off), and logs with `RUST_LOG` (default
`app_web=info,app_core=info,tower_http=info`, which includes the emails
the log mailer writes). The app reads `DATA_DIR`, `SYNC_SERVER_URL` and
`BROWSER_SIGN_IN_URL`, the two URLs also from `tauri/tauri.conf.json`
for a phone build.
`pnpm check` in `svelte/` type-checks the pages.

### Mobile

The Tauri crate is set up for mobile (`tauri::mobile_entry_point`, the
library crate types), and `tauri/gen/apple` is the Xcode project
`cargo tauri ios init` generated, committed as Tauri recommends. Building
for a phone needs:

* Rust through [rustup](https://rustup.rs), not asdf, since the phone
  targets come from it: `rustup target add aarch64-apple-ios aarch64-apple-ios-sim`,
  or for Android the four targets in
  [Tauri's prerequisites](https://tauri.app/start/prerequisites/#configure-for-mobile-targets).
* For iOS: Xcode, selected once with
  `sudo xcode-select -s /Applications/Xcode.app/Contents/Developer`
  (the Tauri CLI runs `xcodebuild` with a clean environment, so
  `DEVELOPER_DIR` doesn't reach it), and CocoaPods, `brew install cocoapods`.
  The CLI installs `xcodegen`, `libimobiledevice` and `ios-deploy` with
  Homebrew itself. A simulator needs no signing certificate; a device
  needs one, set as `bundle.iOS.developmentTeam` in `tauri.conf.json` or
  `APPLE_DEVELOPMENT_TEAM`.
* For Android: Android Studio with the SDK and NDK, and `ANDROID_HOME`
  and `NDK_HOME` set.

Then `bin/dev ios "iPhone 16"` or `bin/dev android`, which run
`cargo tauri android init` the first time (writing `tauri/gen/android`,
to commit) and open the app in a simulator or emulator. The pages come
from the Svelte dev server, which `TAURI_DEV_HOST` (set by the CLI) makes
listen on this machine's network address so a phone reaches it, see
`svelte/vite.config.js`.

## Checks

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
(cd svelte && pnpm check)
```

With `TEST_POSTGRES_URL` set, the core tests run against PostgreSQL too,
see [The database](#the-database).

## Not yet

* TLS to a PostgreSQL server elsewhere: sqlx's `tls-rustls-*` features
* Registering and logging in: today every deployment acts as the dev
  user, see [Who the user is](#who-the-user-is)
* A community's own page, roles, deleting an account: see
  `docs/suggestions/communities.md`
* A bucket of its own for files, the sweep on a schedule, download on
  phones, audio and video: see `docs/suggestions/files.md`
* The `main` branch's desktop app has a quick search popup, a tmux example
  page and the offline-first pages against Phoenix; they can come back
  from there once the pieces they need are here
