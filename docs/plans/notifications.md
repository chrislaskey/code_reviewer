# Plan: notify when a session's agent finishes

Status: steps 1, 2 and 5 built (decisions 29 and 30); step 4's portal setup done. Live Activities are left for later.

## The feature

- Off by default. A session's page gets a bell button at the top right,
  in the Header's `actions` slot where the sessions page has New session.
  It can be toggled at any time during the session; the icon shows which
  (`lucide-bell` / `lucide-bell-off`).
- While the bell is on, a system notification is sent on **every** busy →
  idle, until the bell is turned off or the session is removed (Merge,
  Close, or the tmux session ending).
- The notification's title is the session name; its body is the first
  line of the agent's last message, else "Finished". Clicking it opens
  the session's page.
- No check for whether the session's page is already in view: it always
  notifies.
- **A bell per device.** Turning it on on the Mac notifies the Mac only;
  on the phone, the phone only. Each device's session page shows its own
  bell.
- Later: a lock screen status on iOS (a Live Activity) showing the
  session as Working / Done.

## What we have today

- Status per agent (`AgentSession.status`): `busy`, `idle`, Claude Code
  also `shell`, else `None`.
  - **Claude Code** writes it itself to `~/.claude/sessions/<pid>.json`
    with `statusUpdatedAt`. Reliable.
  - **Cursor's `agent` and Codex** are guessed from the chat file's last
    write (`recent_writes.rs`): busy if written in the last 10 seconds.
    A long tool call that writes nothing reads as idle, then busy again.
- The sessions list cache (`session_summaries.rs`, decision 27) is
  **lazy**: it's found again only when a page asks for it. With no page
  open, nothing looks at the status, so nothing could notice it change.
- Pages poll through `repeatWhileVisible`, which **stops while the page
  is hidden**, which is exactly when a notification is wanted.

So "the cache knows the last status" is half the job: something has to
keep looking while nobody's watching, and remember which sessions want a
bell.

## Option A: detect it in the page (JavaScript)

The session page already polls `get_session` every 5 s. Compare the
previous status with the new one, and on busy → idle call
`@tauri-apps/plugin-notification`.

- For: smallest change; no server state.
- Against: works only while that page is open and visible; the poll
  stops when the window is minimized or the phone app backgrounds. Puts
  application logic in JS, against AGENTS.md. Two devices with the page
  open both notify.
- Verdict: not worth building; it fails the main use case.

## Option B: a watcher in the server (recommended)

A background task in `server/` that keeps looking, decides when a
watched session finished, and hands notifications to whoever delivers
them.

1. **Remembering the bells.** Server side, as the session name → the
   ids of the devices whose bell is on. Choices:
   - **a JSON file in the app data directory**
     (`session_notifications.json`, beside `paired_devices.json`, written
     the same way: private, replaced atomically). Knows nothing of tmux:
     the key is the session name, which any other terminal multiplexer
     or session runner would also have. Survives a restart.
     **Recommended.**
   - a tmux user option on the session (`@code_reviewer_notify`): goes
     away with the session for free, but ties the feature to tmux, and
     holding a list of device ids in a tmux option is awkward.
   - a file in the artifacts directory: no. That directory is for files
     the user looks at; `bin/dev` serves it over HTTP (to the tailnet
     too), it exists only for this repository's sessions so far, and it
     sits in the main checkout rather than the app's own data.
   - in memory in `Backend`: lost on restart.

   Cleaning up, since a file doesn't go away with the session: Merge and
   Close remove the session's entry; the watcher removes entries whose
   session is no longer running (a tmux session ended outside the app)
   the first time it doesn't find them; revoking a paired device removes
   its id everywhere. That matters because Continue makes a new session
   with the old name: without the watcher's cleanup, an old bell would
   come back on.
2. **Knowing the device.** The access middleware checks the token but
   doesn't pass on whose it is. It would add the caller to the request
   (`Caller::Desktop` for the localhost listener's token,
   `Caller::PairedDevice(id)` for a tailnet one), so
   `set_session_notifications_enabled` and `get_session` act on the
   caller's own bell without the client naming itself.
3. **Watching.** A task started in `start.rs` that, while any session
   has the bell on, calls `list_session_summaries` every 1–2 s (sharing
   the cache's refreshes with the pages, so no extra tmux / `ps` cost
   when a page is open too) and keeps the last status of each watched
   session. When nothing is watched it sleeps.
4. **Deciding "finished".** busy → idle, plus:
   - for Cursor and Codex, only after the session has been idle for a
     while (say 30 s), so a quiet tool call doesn't notify; Claude Code's
     own status needs no wait.
   - busy → session gone (the agent exited) counts too.
   - one notification per busy → idle, not per poll.
   - **The message line:** on a finish only, the watcher calls the
     existing `get_agent_chat_history` for that one session and takes
     the first line of the newest `Assistant` message's text, cut to
     about 120 characters. That reuses each agent's transcript reader
     and runs once per finish, not per poll, so it's a small addition.
     If it fails or finds nothing, the body is "Finished".
5. **Delivering.** The server has no Tauri dependency, so it can't post a
   notification itself. Choices:
   - **A long-poll route** `POST /api/wait_for_session_notifications`
     that returns when there is one (or after ~30 s, empty). The desktop
     app's Rust side loops on it with `BackendClient` and posts each with
     `tauri-plugin-notification`. Fits decision 25: everything goes
     through HTTP, so a standalone server later works unchanged.
     **Recommended.**
   - Server-sent events: same idea, streamed; needs streaming in
     `BackendClient`. Slightly better latency, more code.
   - A callback passed to `server::start` from the desktop app: simplest,
     but ties the server to running in the desktop process, which
     decision 25 wants to keep undone.

Each finish goes to the devices in the session's entry. The desktop gets
it through the long-poll. A phone can't long-poll while suspended, so it
gets nothing until push (step 5); until then the bell is shown on the
desktop only, rather than a bell on the phone that does nothing.

### Option B+: push from the agents' own hooks

Instead of, or as well as, polling:

- **Claude Code hooks**: `Stop` (the turn ended) and `Notification`
  (waiting for permission, or idle for a while). Exact, and the second
  one is something polling can't see at all: an agent **waiting for
  approval** reads as idle, yet it hasn't finished. Claude Code's status
  file also lists a `notify_idle` peer feature, undocumented.
- **Codex**: the `notify` setting in `~/.codex/config.toml` runs a
  program on `agent-turn-complete`.
- **Cursor's `agent`**: has hooks too (to check which events the CLI
  fires).

A hook would run a small script that POSTs to the server, which needs a
token, or writes a file the server watches. Either way it means editing
the user's agent settings, which the app doesn't do now (ask first). A
good second step for the cases polling gets wrong, not the first.

### Option B': watch the files instead of polling

Use the `notify` crate on `~/.claude/sessions/` and the chat files, and
look only when one changes. Less work when idle, but Cursor and Codex's
"idle" is the *absence* of writes, so a timer is needed anyway. Not
worth it yet; polling at 1–2 s while something is watched is cheap.

## iOS

The phone's app is suspended soon after it leaves the screen, so it can't
poll, and a local notification from its own code only works while it's
open. Getting anything to a locked phone means **Apple Push Notification
service (APNs)**: the Mac server sends to Apple, Apple wakes the phone.

What that needs:

- An APNs key (`.p8`) from the user's Apple developer account, kept out of
  the repo (`secrecy`, a file in the app data directory).
- The phone registers for remote notifications and sends its device token
  to the server when paired (stored with the paired device, revoked
  with it). Tauri 2 has no official push plugin; this is a small Swift
  plugin in `gen/apple` or a community plugin to evaluate.
- The server sends to `api.push.apple.com` over HTTP/2 (the `a2` crate,
  or `reqwest` with a JWT). **This is the first time the server talks to
  an outside service**, and the notification text (session name, maybe a
  summary) passes through Apple. Worth a decision entry.
- The watcher sends a finish to a phone's token only when that phone's id
  is in the session's entry.

### Testing APNs without the App Store

Yes, all of it can be tested locally, and a private app never has to go
through the App Store or TestFlight:

- **Development builds use APNs' sandbox** (`api.sandbox.push.apple.com`).
  An app signed with a development provisioning profile and installed with
  `bin/run-on-iphone` gets a sandbox device token, and pushes sent to the
  sandbox reach it, locked or not. A private app can stay on the sandbox
  for good; a paid account's development profile lasts a year.
- **The simulator gets real pushes** on an Apple silicon Mac (Xcode 14+,
  macOS 13+): it registers and receives sandbox tokens like a phone.
  `xcrun simctl push <device> com.chrislaskey.codereviewer payload.apns`
  also delivers a payload without Apple, to test how the app shows it.
- **Apple's Push Notifications Console** (in the developer account) sends
  a test push to a device token by hand, to check the key and token before
  the server's code exists.
- **Live Activities** can be started and pushed to in the simulator and on
  the phone the same way (the activity has its own push token).

What setup it needs, once:

- The paid developer account (free accounts can't use push).
- Push Notifications turned on for the `com.chrislaskey.codereviewer` App
  ID, and the `aps-environment: development` entitlement in
  `gen/apple/code_reviewer_iOS/code_reviewer_iOS.entitlements` (empty
  now). Xcode regenerates the profile with it.
- An APNs auth key (`.p8`) with its key ID and the team ID, kept out of the
  repo like `APPLE_DEVELOPMENT_TEAM` is.

Things that catch people out: a sandbox token sent to the production host
(or the other way round) fails with `BadDeviceToken`; tokens change on
reinstall, so the phone sends its token on every launch, not only at
pairing; and the server should drop a token APNs reports as unregistered.

Alternatives to running APNs ourselves:

- **ntfy** (self-hosted or ntfy.sh) or **Pushover**: the server POSTs, their
  iOS app shows it. No Swift and no Apple key, but a separate app on the
  phone and a third party in the path. Fine as a stopgap.
- **Tailscale** has no push channel.

### Lock screen status (Live Activity)

ActivityKit's Live Activities show "cr-Alerts · Working… 4 min" on the lock
screen and Dynamic Island, then "Done".

- Needs a Swift widget extension target in the Xcode project (`gen/apple`),
  outside what Tauri generates, and a bridge plugin to start it.
- Started from the app while it's open (tapping the bell could start one),
  and then updated **only by APNs pushes** to the activity's own push
  token while backgrounded. So it depends on the APNs work above.
- Lasts up to 8 hours on screen, 12 in all; iOS limits update frequency.
  Busy ↔ idle changes are rare enough.
- Order: APNs notifications first, Live Activity after.

## Steps

Each one small and shippable.

1. **Bell and watcher, desktop only.** `session_notifications.json` and
   its cleanup; the caller added by the access middleware; `get_session`
   returns the caller's bell; `set_session_notifications_enabled` on
   `Backend` / route / client / Tauri command / `backend.js`; bell button
   on the session page; watcher task; long-poll route; desktop loop
   posting with `tauri-plugin-notification`, titled with the session
   name and the last message's first line; click opens the session page.
   **Built.**
   Tests for the busy → idle decision (pure function over two snapshots
   plus timing) and for picking the message line. Decision entry.
2. **Bell on the session cards** (index and the More sheet), read only,
   so you can see which sessions are watched. **Built:** the sessions list
   says whether the asking device's bell is on for each session.
3. **Waiting for permission** via Claude Code's `Notification` hook, if
   step 1 shows it's missed in practice.
4. **APNs setup, checked by hand.** App ID capability, entitlement, `.p8`
   key; a test push from Apple's console to the simulator and the phone.
   No server code yet. Portal part done; see
   `docs/guides/apple-push-notifications.md`.
5. **iOS push from the server.** The phone registers and sends its token
   on launch (stored with its paired device, removed on revoke); the
   bell shows on the phone; the watcher sends finishes to the sandbox
   APNs host for each phone whose bell is on. **Built** (decision 30); the
   tokens are kept beside the bells instead of with the paired device, as
   only remote access holds those. Waits on a test on the phone.
6. **iOS Live Activity.**

## Gaps and questions

- **Cursor and Codex false alarms.** The 10 s write window will fire on
  quiet tool calls; the 30 s settle helps but delays the real ones. The
  hook route (B+) fixes it properly.
- **macOS permission.** The first notification prompts for permission;
  dev builds run as a different bundle than release, so it may prompt
  twice. Notifications from `bin/dev` may show under "Terminal" or not at
  all until the app is bundled; to check early.
- **Server restarts.** The bells survive in the file; the watcher's
  last-status memory doesn't, so a session that finishes during a restart
  is missed. Acceptable.
- **Notification text through Apple.** Once APNs is in, the session name
  and the message line pass through Apple's servers. A setting to send
  only "A session finished" to the phone could come later if that
  matters.
- **Standalone server.** With the long-poll design, a launchd server with
  no desktop app running delivers nothing on the Mac; only push (step 4)
  would reach anyone. Fine for now.
