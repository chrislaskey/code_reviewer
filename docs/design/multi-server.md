# Signing in to another server from the app

**Status: agreed and built, 2026-11-22, but for what "Not built yet"
lists.** The design as discussed; where the code differs in a name, the
code is right.

Most apps built from the template have one web server, compiled into
the desktop and mobile builds, and every account on a device is an
account there. Some apps want a device to sign in to other deployments
of the same app too, run by a business or by the person themselves.
This is that: an account on a device belongs to one server, and the app
talks to the active account's server alone.

The web is unaffected: a browser tab is on one server by nature.

## Settled

- **One server at a time.** Only the active account syncs, as today,
  see `device-accounts.md`; switching to an account on another server
  is a switch like any other. No background sync, alerts or badges from
  other servers.
- **The device keeps a table of servers**, `device_servers` in
  `device.db`, and each account names its server. Not a URL on each
  account row: what the app learns about a server, its name, whether it
  is listed, when it was checked, is the server's, shared by its
  accounts.
- **Servers describe themselves**, `GET /api/servers/current`: the app
  it runs, its sync protocol version, its name (a slug, `acme`) and its
  display name. The app asks before opening the sign-in page, which
  catches a typo, a server that is down, and a server of a different
  app built from the template, whose synced tables would not match.
- **The default server says whether it lists a server**, from a static
  list in its config of names and origins: `GET /api/servers/:name?url=`
  answers the entry only when both the name and the origin match, and
  404 otherwise, the same for an unknown name as for a wrong origin. So
  the list of an app's customers is never published, and neither half
  alone finds anything. `current` is reserved, never a server's name.
- **Trust is the origin the app connects to**, never what a server says
  of itself: a malicious server can claim a listed server's name, but
  cannot answer over TLS at that server's origin, which the app sends
  with the name. The display name is for display only.
- **One build-time setting**, `multiServer` in the `starter-pack`
  entry of `tauri.conf.json`, compiled in so the person cannot change
  it and so it reaches phone builds:
  - `none`, the default: today's behaviour, nothing new on any page.
  - `listed_only`: only servers the default server lists.
  - `any`: any server; an unlisted one is shown as such, and the person
    goes on at their own risk.
- **The app always asks the default server** whether a server is
  listed. An app that must not depend on a central server, or needs
  stricter control than this, ships its own builds with its own default
  server, as the template does today; that logic is not built into the
  app.
- **Purchases are hidden on accounts of other servers.** App Store
  Connect and the Play Console send their notifications to one URL per
  app, and verifying a purchase takes the app's own store keys, so a
  phone's billing reaches the default server alone. Desktop purchases
  are hidden too, for one rule everywhere.

## The servers on a device

`device_servers`, one row per server the device knows:

| column | |
| --- | --- |
| `id` | made on the device; what an account names |
| `url` | the origin, normalized (`https://host[:port]`, lowercase, no path); NULL for the build's default server |
| `name`, `display_name` | as `/api/servers/current` gave them |
| `app_identifier`, `protocol_version` | as `/api/servers/current` gave them |
| `listed` | `default`, `listed` or `unlisted` |
| `checked_at` | when `/api/servers/current` and the listing were asked |

The default server's `url` is NULL and found at runtime, from
`server_urls` as today, never stored: `bin/dev` changes its ports each
session, and a production domain may move.

`device_accounts` gains `server_id`, unique with `user_id`: two
servers made from one database, staging restored from production, have
the same users with the same ids. Each row has an id made on the
device, which names its folder and which the pages switch and remove
by, so the `file` column is gone. A local user has no server,
`server_id` NULL, and keeps the `local` flag: a move into an account
names the account's server on the row first, then changes the file,
so a move cut short is finished at start on the right server, while
the row is still the local user's.

`device_sign_in` gains `server_id`, so a sign-in finished after a
relaunch, or a signed-out account signing in again with its email
filled in, exchanges its code with the server it started on.

## How the app talks to the active server

Today `sync::setup` builds one `Http` transport when the app starts and
keeps it in `SyncServer`, which some 25 commands ask for. Instead, the
active account in `DeviceAccounts` holds its `Http` beside its
`Deployment`, built when it is activated; `SyncServer` keeps only the
lock one sync at a time holds. A switch already holds that lock, so the
file and the server change together, and a sync in flight finishes
against the server it started on.

`server_urls` answers for a server: the sign-in page, the loopback's
`back-to-app` page, the terms and privacy pages and the Stripe pages
are the active account's server's; before anyone is signed in, the
default server's.

## What the app trusts less on another server

A server pushes rows and hands the device URLs. On the default server
both are the app's own; on another, they are not. The app:

- accepts only https, but `localhost` in dev builds; iOS refuses plain
  http anyway;
- shows the host before the sign-in page opens, and on every account
  row, so two accounts with one email are told apart;
- opens in the browser only URLs on the account's server, or on the
  hosts the app itself knows, such as Stripe's;
- relies on the baseline validation of each table, see
  `low-level-contexts.md`, for what it pulls.

## The flows

- **Adding an account on another server**, on `AccountsIndexPage`,
  only when `multiServer` is not `none`: "Custom server" shows a
  URL field. The app normalizes it, asks that server's
  `/api/servers/current` for its name, asks the default server's
  `/api/servers/:name?url=` with that name and the origin, and shows
  the host, its display name and whether it is listed. It refuses when:
  - the server answers for a different app, or a protocol version the
    app cannot sync;
  - `multiServer` is `listed_only` and the server is not, or the default
    server cannot be reached to ask: it says it cannot check the server
    now.

  With `any`, a server not listed, or not checked as the default server
  could not be reached, is shown so, with a way on. Then the sign-in
  goes on as today, in the browser at that server's sign-in page.
- **The listing is asked once**, when the server is added. Switching
  accounts and signing in again ask nothing, so a switch stays as fast
  as today; there is no periodic check.
- **Sync, sign-out, a 401, a 410, account deletion** go to the active
  account's server, by its transport, with no change to their logic.

## The protocol version

The app syncs with a server whose protocol version is its own,
`sync::PROTOCOL_VERSION`, which starts at `1`. A server of another
version is refused when it is added, by its description. After that,
every request of the device carries its version, in the
`x-protocol-version` header, and the server's sync routes refuse
another, or none, with a 426 whose words name the side to update. The
device reports it as the sync's error, `SyncError::ProtocolVersion`,
on every sync until one side is updated, while the person goes on on
the device as when offline: their changes wait, and the first sync
after the versions match pushes them. What changes the version, and
whether a range is ever accepted, is decided at its first change.

## The current server

The pages read the server they are on as they read the current user:
`serversCurrentGet`, `{display_name, host, default}`. On the web it is
`GET /api/servers/current` of the server the pages came from; in the
app, `servers_current_get`, the active account's server, else the
default one. The account page shows purchasing only where `default` is
true; the Rust buy commands refuse elsewhere all the same. Only that
page reads it so far, through `loaded()`; it becomes a shared `$state`
module once a second component, such as the nav, does.

## The details

- **The app a server runs** is `APP_IDENTIFIER` in
  `core/src/app/identity.rs`, a seam every fork edits with the
  `identifier` of `tauri/tauri.conf.json`; a test of the Tauri app
  keeps the two equal.
- **A server's own name**: `SERVER_NAME`, a slug, and
  `SERVER_DISPLAY_NAME`, read in `axum/src/config.rs`; unset, `default`
  and the product name, so a deployment of today needs no change.
- **The list**: `LISTED_SERVERS`, entries of a name and an origin,
  `acme=https://acme.example,beta=https://beta.example`, on the default
  server, set as a Fly secret where it runs there. A malformed entry, or
  one named `current`, stops the server at start.
- **A typed URL** is normalized to its origin: https only, but
  `localhost` and `127.0.0.1` in dev builds; the host lowercased, the
  port kept unless it is 443. A path, query, fragment or
  `user:password@` is refused, never dropped.
- **Rate limits**: `/api/servers/:name` is behind the same per-IP limit
  as the accounts routes, so names cannot be guessed quickly;
  `/api/servers/current` is open.
- **Dev data**: step 1 edits the `device.db` migrations in place and
  names the folders anew, so each worktree runs `bin/reset` after it.

## Later

- Universal links and deep links to sign up on a given server. Universal
  links and app links are compiled into the build for its domains, so a
  link for another server is a link on the default server's domain that
  names it, or the app's scheme; it goes through the same checks.
- A list in the default server's database with an `ops` page, in place
  of the static config.
- Push notifications, which hit the same wall as purchases: the APNs
  and FCM keys are the app's.
- Self-signed certificates for servers on a business's own network.
- Refreshing what a kept server said of itself, its display name and
  its listing, after a sync when the last check is a day old. Left out
  while nothing acts on the listing after a server is added.
- Apps with no default server, where every server is the person's own,
  as Code Reviewer's phone app pairs with a Mac today: `multiServer =
  any` with no default configured skips the listing, shows no warning
  and no default sign-in. Not built until such an app moves onto the
  template.
- Pairing as a way to sign in: a signed-in device makes a short-lived
  code for its own user, which another device exchanges for a token of
  its own, for servers with no mailer such as a person's own Mac, and
  for signing in a phone from the desktop.

## Build order

Each step leaves every check green and the app working.

1. **Servers on the device.** `device_servers`, `server_id` on
   `device_accounts` and `device_sign_in`, folders by a device id, the
   transport in the active account, `server_urls` per server. Only the
   default server's row exists; nothing changes for the person.
2. **The server describes itself.** Built: `app_core::servers`,
   `axum/src/servers.rs`, and the Tauri command `servers_get`, which
   checks a typed URL and keeps nothing.
3. **Another server on the accounts page.** Built: the `multiServer`
   setting, `MULTI_SERVER` in `bin/dev` for a session; "Custom server",
   `AccountsCustomServer`, which checks the URL with `servers_get`, shows
   the host, the display name and the listing with its warning, and on
   confirmation keeps the server, `device_servers_create`, which checks
   it again, and signs in there, `accounts_sign_in_start` with its
   server; the host on each account row; purchasing hidden on another
   server's account, by the current server, and the buying commands
   refusing there.

## Not built yet

- **Opening only the account's server's URLs in the browser.** The
  URLs a server hands the app to open are Stripe's checkout and portal
  pages, both refused on another server's account, where nothing is
  sold; a general check waits for the first other URL a server hands
  over.

These are framework files: the change goes to the template in its own
commits, with `cargo run -p framework_check -- lock`, and the
`device.db` migrations are edited in place, as nothing is deployed.
