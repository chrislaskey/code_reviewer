# Moving Code Reviewer onto the Starter Pack template

Status: investigating, started 2026-11-23, branch `cr-v4-starter-pack`.
`main` is the reference for how Code Reviewer works today.

## Goal

Code Reviewer becomes an app built from the Starter Pack template
(`../starter_pack`), as `docs/design/template-and-apps.md` there describes:
a copy with the template as a `template` git remote, pulling updates with
`bin/framework update` and sending generic changes back with
`bin/framework contribute`. Gaps found along the way are closed in the
template where they serve every app.

## What each is today

| | Code Reviewer (`main`) | Starter Pack (`main`, ea5f927) |
|---|---|---|
| Crates | `core/` (analysis), `server/` (host backend), `client/` (`BackendClient`), `tauri/src-tauri/` | `core/` (`app_core`), `axum/` (`app_web`), `tauri/` (`app_tauri`), `framework_check/` |
| The server | runs **on the user's Mac**, started by the desktop app: tmux, git worktrees, agents' chats, the Mac's files | runs **in the cloud** (Fly.io), the system of record |
| How it's reached | `POST /api/<command>`, a token per client; localhost for the desktop, the tailnet (TLS, `whois`, pairing codes) for phones | `GET/POST/PUT/DELETE /api/<resource>`; a browser by cookie, the apps by bearer token after signing in |
| The desktop and phone apps | every command a `BackendClient` call to the Mac's server; the phone has no logic of its own | `app_core` in process on a SQLite file per account, synced with the web server, works offline |
| Data | JSON files in the app data directory (tokens, paired devices, remote access, session caches); nothing synced | SQLite or PostgreSQL, migrations, events, sync, conflicts, purge, jobs |
| People | one user, the Mac's owner; devices paired | accounts, sign-in by emailed code, local and trial users, communities, purchasing, ops |
| Web | none | the API, public pages and the Svelte app in a browser |
| Pages | `tauri/src/`, own components (`Header`, `Toolbar`, `ChoiceMenu`, `SegmentedControl`, ...), `backend.js` | `svelte/src/`, atoms in `lib/components/`, `lib/layouts/`, `loaded()`, `lib/core/` choosing Tauri or HTTP, every string through ttag |
| iOS | APNs pushes, no keyboard bar, Tailscale pairing | StoreKit, web auth session, clipboard, quick picker |
| Tooling | `bin/check`, `bin/dev`, `bin/reinstall`, `bin/worktree`, `bin/artifacts` | `bin/lint`, `bin/dev`, `bin/reinstall`, `bin/worktree`, `bin/reset`, `bin/storage`, `bin/framework` |

The two have no git history in common: Code Reviewer copied the
template's `tauri/` directory in October (decisions 1 to 3), before the
template's Axum, sync and accounts came back.

## Direction, decided 2026-11-23

The template's web server (`axum/`, `app_web`) becomes Code Reviewer's
backend, running on the Mac, reachable only on the tailnet. The desktop
and phone apps are both clients of it. This was option 2 of three; the
others were keeping `server/` beside a cloud server (1) and taking only
the frontend and tooling (3).

- **Tailscale stays the boundary.** Never a public server or relay: the
  backend runs agents that execute code. The server listens on
  localhost and the Mac's tailnet address, with the `whois` check per
  connection and TLS from `tailscale cert`, as today.
- **Two processes, not one.** The server runs on its own (`app_web`); the
  desktop app no longer starts it, and connects to it like the phone.
  A Mac, or later a headless Linux box, can run the server with no
  desktop app.
- **People have accounts.** Sign-ups off (`features.accounts_sign_up`),
  accounts made by hand on the server, sign-in by password only, no
  emailed codes. These replace pairing codes and per-device tokens.
- **Not now:** Linux, more than one Mac (the template's
  `multi-server.md` is the path, two Macs the first test), launchd.
- **The web is for signing in.** Phones and the desktop sign in through
  the server's sign-in page, so it stays; everything else is used
  through the apps, and the host routes take the apps' bearer token, not
  the browser's cookie. The rest of the web pages may be turned off
  where that is easiest.
- **The host code lives in `core/`**, behind a `host` Cargo feature that
  only `axum/` turns on, as `stripe` and `app-store` are today. Not a
  crate of its own; the feature keeps tmux, git and the tree-sitter
  grammars out of the phone build, and the compiler stops the app's side
  from calling them.
- **English only at first.** The template's translation check is turned
  off while the pages move over (a `[[divergence]]` if it lives in a
  framework file); putting the pages through ttag is a later step.
- **Host operations are online only.** Reviews, traces, sessions, tmux and
  worktrees run on the server's machine; the apps ask the server, the
  template's pattern for online-only calls (`app_core::accounts::online`,
  `multi-app.md`).

### What the template has already

- The web server as its own process, `HOST` and `PORT`.
- Turning sign-ups off per server, `core/src/app/features.rs`.
- Passwords (Argon2id), bearer tokens for the apps, device sign-in
  through the browser with a challenge.
- Online-only Tauri commands that pass the server's JSON on.
- `ops staff create <email>` on the server's command line.

### Gaps in the template, generic

1. **Sign-in by password only.** Built 2026-11-23 on Starter Pack's
   `cr-Passwordonly` branch: `features.accounts_sign_in_code`, off on a
   password-only server.
2. **Making an account from the command line.** Built on the same branch:
   `app_web accounts users create <email>`.
3. **Listening on the tailnet**: a second listener on the Tailscale
   address with TLS and `whois`, from Code Reviewer's
   `tailnet_listener.rs`, `tailscale.rs` and `access.rs`. Generic for any
   app run as a private server.
4. **Browser sessions on a host that runs code.** The template's pages
   sign in a browser with a cookie (`SameSite=Strict`). Another site on
   the same host but a different port, such as an artifact page served
   by `bin/dev`, counts as the same site. Decided: host operations take
   the apps' bearer token only. How a route says so is still to design.
5. **An app that is mostly online.** The online-only pattern exists for
   accounts; to check it carries a whole app's pages well (loading,
   errors, being offline).

### Gaps in Code Reviewer, its own

- The desktop app starting nothing: `bin/dev` runs the server, and the
  desktop and phone sign in to it.
- Pushes to phones (APNs, `apple_push.rs`): the template has none.
- What goes: `client/` (`BackendClient`), `connection.rs`, pairing codes,
  `paired_devices.json`, the desktop token file.
- The routes that act on the Mac's screen
  (`show_session_in_terminal_app`, `open_session_in_browser`), kept off
  the tailnet today: who may call them once every client is signed in
  the same way.
- What of the template to turn off or delete: trial users, communities
  in the pages, posts and files (examples), purchasing, public pages.

## How the code arrives

Proposed: merge the template into this branch with
`--allow-unrelated-histories`, taking the template's tree, so both
histories are kept and `bin/framework update` works from then on. Then
bring Code Reviewer's features in one at a time, as app-owned files,
each step passing `bin/lint` and the tests, with `main` as the reference.

Considered: two directories side by side until they match. Easier to
start, but the merge with history would still have to happen at the end.

## Reaching the server on the tailnet

What Code Reviewer does today (`tailnet_listener.rs`, `tailscale.rs`,
`access.rs`), in three parts:

1. **Where it listens.** A second listener on the Mac's Tailscale IPv4
   address, port 7443, found with `tailscale status --json`. Not
   `0.0.0.0`, so a café's Wi-Fi never reaches it.
2. **The certificate.** `tailscale cert` gets a Let's Encrypt certificate
   for the Mac's MagicDNS name (`mac.tailnet.ts.net`); the tailnet must
   have HTTPS turned on. WireGuard already encrypts the traffic, but iOS
   refuses plain HTTP to anything but localhost (App Transport Security;
   the app declares no exception), so the phone needs HTTPS regardless.
3. **The check per connection.** Before the TLS handshake the peer's
   address must be a Tailscale one (`100.64.0.0/10`,
   `fd7a:115c:a1e0::/48`), and `tailscale whois` must name a device owned
   by **the Mac's own Tailscale user**. Then the token is checked as on
   localhost.

Part 3 assumed one person. For a trusted team, a teammate's phone is
another Tailscale user and would be refused. With accounts, the check
that matters is the account; the tailnet's access rules decide which
devices reach the Mac at all.

Options:

- **A. Move the listener into the template**, as a setting of `app_web`:
  parts 1 and 2, and part 3 relaxed to "a device of this tailnet".
  About 600 lines from Code Reviewer, generic for any app run as a
  private server.
- **B. `tailscale serve` in front of `app_web`.** The server listens on
  `127.0.0.1` only; `tailscale serve --bg --https=443 http://127.0.0.1:4000`
  gives the tailnet HTTPS with the certificate renewed by Tailscale, and
  passes only tailnet traffic. No listener code in either repository,
  the same on Linux. Costs: every request arrives from `127.0.0.1`, so a
  route meant for the Mac's own desktop (showing a session in Terminal)
  tells tailnet callers apart by the `Tailscale-User-Login` header
  `serve` adds, and the rate limits need `TRUST_PROXY_HEADERS`.
  Decision 25 turned `serve` down when the backend had no accounts; with
  accounts and bearer tokens, its reasons are gone.
- **C. Keep the listener as Code Reviewer's own** code. Least template
  work, but the template could not run a private server by itself.

Decided 2026-11-23: B, tried on the Mac first; A if `serve` falls short.
The "same Tailscale user" rule goes: any device the tailnet lets reach
the Mac may try, and the account decides.

## Inventory

Where each part of Code Reviewer goes in the template's layout. "Host"
is the `host` Cargo feature of `core/`, on only in `axum/`.

### Rust

| Code Reviewer (`main`) | Goes to | Notes |
|---|---|---|
| `core/src/analysis/`, `groups/`, `git.rs` (reviews and traces) | `core/src/app/reviews/`, `core/src/app/traces/`, host | Moved as is: plain functions with their tests. tree-sitter, `comrak`, `rayon` become optional dependencies of the feature |
| `server/src/` tmux, git worktrees, terminal app, processes, and the Claude Code, Codex and Cursor clients | `core/src/services/`, host | Clients of other programs, as `services/` is for other services. Needs gap T1 |
| `server/src/sessions.rs`, summaries, finished sessions, continue, recent writes, artifacts | `core/src/app/sessions/`, host | The high-level operations the routes call |
| `server/src/backend.rs` (`Backend`, one method per command) | gone | Each method becomes a function in `core/src/app/`, as in the template |
| `server/src/http_api.rs`, `requests.rs` | `axum/src/app/reviews.rs`, `traces.rs`, `sessions.rs` | Routes renamed by resource (`GET /api/sessions`, `POST /api/sessions/:name/prompts`), signed in by bearer token only. Needs T1 |
| `server/src/start.rs` background tasks (session summaries, finished sessions) | a seam in `axum/` for an app's own tasks | Long-running watchers, not the template's 15-minute jobs. Gap T2 |
| `server/src/session_notifications.rs`, `apple_push.rs` | `core/src/app/sessions/notifications.rs`, `core/src/services/apple_push.rs`, host | Kept per device today, by its paired device id; becomes per signed-in device, the token's row. Gap C1 |
| `access.rs`, `tokens.rs`, `paired_devices.rs`, `pairing_code.rs`, `remote_access.rs`, `tailnet_listener.rs`, `tailscale.rs` | gone, or the template (option A above) | Accounts and the tailnet decision replace them |
| `client/` (`BackendClient`) | gone | The template's `Http` with the device's token (`app_core::services::web_server`) |
| `src-tauri/src/review.rs`, `trace.rs`, `sessions.rs` | `tauri/src/app/reviews.rs`, `traces.rs`, `sessions.rs` | Each command one online-only call, as `accounts::online` does. Needs T1 |
| `src-tauri/src/connection.rs`, `remote_access.rs` | gone | Device accounts and their server |
| `src-tauri/src/notifications.rs` (desktop: asks the server for notifications, shows them) | `tauri/src/app/notifications.rs` | Needs `tauri-plugin-notification` |
| `src-tauri/src/apple_push_registration.rs` | `tauri/src/app/` | Phone only |
| `src-tauri/src/keyboard_accessory_bar.rs` | the template | Any app's forms on iOS want it. Gap T3 |
| JSON files in the data directory | see C2 | `desktop_token`, `paired_devices.json`, `remote_access.json` go; the APNs key and settings become server config |

### Pages

| Code Reviewer (`tauri/src/`) | Goes to (`svelte/src/`) | Notes |
|---|---|---|
| `HomePage.svelte` | `home/HomePage.svelte` | The template's home is a seam |
| `reviews/` (new, show, ledger, nodes; SvelteFlow) | `reviews/` | `@xyflow/svelte`, `highlight.js`, `d3-*` join `svelte/package.json` |
| `traces/` (new, show, call stack) | `traces/` | |
| `sessions/` (index, new, show, chat, menus, bell) | `sessions/` | The largest part |
| `remote_access/RemoteAccessPage.svelte` | gone | The template's accounts page and its custom server sign-in |
| `lib/backend.js` | `lib/core/tauri-commands.js` | `http-api.js` answers the host calls with "use the app" (web off) |
| `lib/routes.js`, `router.js` | the template's (seams) | |
| `lib/components/` own pieces: `Toolbar`, `ChoiceMenu`, `MenuChoice`, `SegmentedControl`, `InfoPopover`, `CodeLines`, `ComparisonFields`, `NavigationHistoryButtons`, the waves and ribbons | `lib/code_reviewer/` or each page folder's `components/` | Not into `lib/components/`, the template's atoms (T1). `Header`, `Button`, `Icon`, `ThemeChoices`, `ThemeMenu`, `AppNav`, `BottomBar`, `RemountPageOnNavigation` have template versions; each is compared, and what Code Reviewer added goes to the template if generic |
| `lib/` helpers: `syntax_highlight.js`, `change_styles.js`, `flow_lines.js`, `comparison_form.js`, `code_page.css`, `log.svelte.js` | `lib/code_reviewer/` | T1 |

### Tooling and docs

| Code Reviewer | Template | Notes |
|---|---|---|
| `bin/check` | `bin/lint` plus `cargo test` | `bin/check` goes |
| `bin/dev` | `bin/dev` | The template's starts the server and the pages; the desktop app no longer starts the server |
| `bin/reinstall`, `bin/worktree` | same names, already in step | |
| `bin/artifacts` | app-owned script | Needs T1 (`bin/**` is all framework) |
| `docs/decisions.md`, `docs/suggestions/`, `docs/guides/`, `docs/plans/` | kept | `docs/` is the app's |
| `AGENTS.md`, `README.md` | the template's, with Code Reviewer's sections | |

## Gaps the inventory found

In the template, generic:

- **T1. An app's new files in the template's directories.** Done
  2026-11-23, Starter Pack `8a3e4d7`. `framework.toml`
  claims whole directories: `axum/src/**`, `tauri/src/**`,
  `core/src/services/**`, `svelte/src/lib/**`, `bin/**`. A file an app adds
  there, such as `axum/src/app/recipes.rs`, is reported "added" and fails
  `cargo test`. Any app hits this with its first route. Fix: framework
  globs name the template's own files and folders, leaving `app/`
  folders and an app's new files to the app.
- **T2. An app's own background tasks on the server**: done 2026-11-23,
  Starter Pack `0e3f24f`, a `ServerTask` listed in
  `core/src/app/server_tasks.rs`, started with the server and started
  again when it fails.
- **T3. No bar above the keyboard on iOS**: Code Reviewer's
  `keyboard_accessory_bar.rs`, for every app. Done 2026-11-23, Starter
  Pack `8a3e4d7`; compiled for iOS alone, not yet run on a phone.
- **T4. Reaching the server on the tailnet**: option B, `tailscale serve`.
- From before: browser sessions refused on host routes (gap 4), and the
  online-only pattern carrying a whole app (gap 5).

In Code Reviewer, its own:

- **C1. Which device a notification goes to.** Bells and push tokens are
  per paired device today. With accounts, the device is its signed-in
  token's row; when it signs out, its bells and push token go with it.
  Decided 2026-11-23: bells stay per device; every signed-in person sees
  every session (sessions of one's own, a suggestion for later).
- **C2. Where host state is kept.** Notification bells, push tokens and
  session caches are JSON files today. Decided 2026-11-23: kept as files
  in the server's data directory to start; tables later only if needed.
- **C3. The Mac-only route**, showing a session in Terminal: decided
  2026-11-23, refused to a request carrying the `Tailscale-User-Login`
  header `tailscale serve` adds, so only the Mac itself calls it, as
  today. Creating and continuing a session from a phone still open its
  window on the Mac. Open: a server with no terminal app (headless
  Linux), where opening the window fails today.
- **C4. The phone finding its server.** The pairing code carried the URL.
  Decided 2026-11-23: the template's custom server sign-in
  (`multiServer = "any"`), the person typing the Mac's
  `https://mac.tailnet.ts.net` once.

## Order of work

1. Template: T1, then T4 tried on the Mac. T2 and T3 when their feature
   arrives.
2. This branch: merge the template (`--allow-unrelated-histories`),
   taking its tree; Code Reviewer's `docs/` kept. Password only, sign-ups
   off, `multiServer = "any"`, the app's name and bundle id, the examples
   (posts, files) and purchasing removed, translations check off.
   Merged 2026-11-23 (6d15290); `phoenix/` deleted. What it found:
   - Sign-ups and codes are `SERVER_SETTINGS`, not `features.rs`
     defaults, which framework tests rely on. Template: `bin/dev` reads
     it from `.env` (T5).
   - `bin/reinstall` named the template's app; it reads
     `tauri.conf.json` now (T6). `APP_IDENTIFIER` in
     `core/src/app/identity.rs` must match the bundle id.
   - Purchasing is framework: turned off with
     `features.accounts_purchasing` (T7), not deleted.
   - The examples can't be deleted: framework code and tests name them,
     and the quick picker, kept, uses files. Hidden from the nav only;
     the template's fix is its `docs/suggestions/code-organization-examples.md`.
3. Reviews and traces: read-only, no background tasks, the smallest
   feature that crosses every layer. Built 2026-11-24, all but the check
   on a running app: template `CurrentUserFromBearerToken` and
   `CurrentUserFromCookie`, the `ServerState` seam, `services.rs` and
   `svelte/src/lib/pages.js` as seams; `host` feature with
   `core/src/app/analysis/`, `groups/`, `services/git.rs`; routes under
   `/api/reviews/*` and `/api/traces/*`; online calls, Tauri commands,
   both JS clients; pages in `svelte/src/reviews/`, `traces/`, own
   components in `svelte/src/lib/code_reviewer/`. Main's waves and
   background pattern left out for now. The first proof of the host feature,
   the online-only calls and a phone signed in over the tailnet.
4. Sessions, without notifications. Built 2026-11-24, all but the check
   on a running app: `core/src/services/` (tmux, git worktrees and status,
   processes, terminal app, `agents/`), `core/src/app/sessions/` with
   `SessionHost` in `ServerState`, routes under `/api/sessions` (the
   terminal window refused over the tailnet), online calls, Tauri
   commands, pages. Artifacts kept as on main at first; since 2026-11-24 the
   server serves them itself at `/artifacts/`, sandboxed (6b).
5. Notifications: desktop, then APNs. Built 2026-11-24, all but the check
   on a running app and a phone: template `ServerTask::run` gets
   `ServerState`; bells per device, the hash of its bearer token, dropped
   once its token no longer signs in; the bells, push tokens and APNs key
   in `axum/data/`; the finished-sessions watcher a `ServerTask`; desktop
   notifications with `tauri-plugin-notification`; the phone's push token
   with `tauri-plugin-notifications` and the `aps-environment`
   entitlement. Found: `framework_check` fails on the `Package.resolved`
   files an iOS build writes in `tauri/plugins/*/ios/`, though git ignores
   them; the check should skip ignored files (template).
6. iOS details; the web turned off but for signing in. Planned below.

## Step 6: the phone, and the web for signing in alone

Planned 2026-11-24, to start after the verification pass of steps 3 to
5, which will add to it. Each part a commit; the ones marked "decide"
need the user's choice first.

### 6a. The phone reaching the server

Built 2026-11-24 (template `bcd10f1`): `bin/dev`, with the web target,
when Tailscale is installed and up, builds the pages and runs
`tailscale serve --https=<web port> http://127.0.0.1:<web port>` in the
foreground, so it stops with the rest; `TAILSCALE_SERVE=false` turns it
off. The phone signs in at the server's own built pages, and one address
carries the pages, the API and the artifacts. Tried on the Mac: the
foreground serve on a port other than 443 answers and leaves no config
behind. Left to check with the phone:

- The `Tailscale-User-Login` header reaching the server (the Terminal
  route's 403), and `TRUST_PROXY_HEADERS` for the rate limits.
- **Installing:** `SYNC_SERVER_URL` in `.env` set to the address
  `tailscale serve` prints (`https://nyx.tailcbcf2d.ts.net:4030` with
  `PORT=4030`), then `bin/reinstall`; `multiServer "any"` for typing it
  in instead (C4).

### 6b. What the phone opens

- **Artifacts:** built 2026-11-24. The server serves them at
  `/artifacts/` from the main checkout of the repository it runs in,
  whichever worktree (`core/src/app/sessions/artifacts.rs`,
  `axum/src/app/sessions/artifacts.rs`), sandboxed by
  `Content-Security-Policy: sandbox allow-scripts ...` so an artifact's
  scripts never act as the signed-in pages, with
  `Access-Control-Allow-Origin: *` for the index's list. The session's
  artifacts link is a path the app opens beside its sign-in page; the
  Vite plugin is gone and the dev server sends `/artifacts` on to the
  server. Left to check with the phone.
- **A session's browser link** is `http://localhost:<its PORT>` on the
  Mac, useless on the phone; on main the app pointed it at the paired
  computer's tailnet name. Decide later, with the phone in hand.
- **The Terminal window** answers 403 over the tailnet: check the page
  says so plainly, and hide the action on a phone.

### 6c. The phone's look and feel

- **Compare main's mobile pieces with the template's** and send what is
  generic to the template: the bottom bar (main: Home, Back, Forward,
  More with a sheet), the top spacing (main's cr-Mobilevertical), and
  `webview_background.js`, which paints the webview in the theme's
  colour behind the status bar and home indicator (the template has
  none).
- **The keyboard bar** (T3): confirm it is hidden on the phone.
- **Code Reviewer's icons and launch screen** in place of Starter
  Pack's (`tauri/icons`, `gen/apple/Assets.xcassets`).
- **Notifications:** the APNs key copied to `axum/data/`, a push
  arriving, and a tap opening its session.

### 6d. The web for signing in alone

The host routes already refuse the browser's cookie, so the web cannot
run anything; what is left is what a visitor sees.

- **The public pages** (`svelte/src/public/`, Code Reviewer's to edit):
  Starter Pack's home, about, privacy and terms. Recommended: one short
  home page naming Code Reviewer and saying it runs in its apps, and the
  rest removed from the build where that is easy.
- **The app's pages in a browser**: left as they are, each host page
  showing its 401 as a sign-in error. Recommended: no gating code (the
  user's guidance, 2026-11-24); the browser's home page says to use the
  app.
- **Template features not used**: communities, posts and files stay out
  of the nav (step 2); trials are off with sign-ups.

