# Accounts on a device

**Status: built, 2026-11-08, as settled below.** The code is
`tauri/src/device_accounts.rs` with its tests, the page
`svelte/src/accounts/AccountsIndexPage.svelte`, and the two core changes
in `core/src/accounts.rs`. The sections after "Settled" are the design
as it was discussed and then built; where the code differs in a name,
the code is right.

Today a device holds one SQLite file, `app.db`, one `Deployment` managed
as Tauri state, and one `signed_in` device setting inside that file
naming the user and holding their token. A sign-out tells the server to
forget the token and wipes the file, see "Who the user is" in
`offline-sync.md`. So two accounts on one phone resync their data from
nothing on every switch. The web is one browser session, one cookie,
and stays that way: nothing here applies to the web.

Since 2026-11-11 one of the accounts may be a **local user**, made by
Skip with no account on the server, see
`accounts-local-and-trial-users.md`: `device_accounts.local`. It lists,
switches and removes like any account and has no sign-out. When its
records move into an account, the row becomes that account's and keeps
its folder, so a folder is found by the row's `file`, no longer by the
user's id, and the sweep keeps whatever folder a row names.

## Settled

- **One SQLite file per account.** Each account on the device gets a
  complete device database of its own, exactly what `app.db` is today:
  its own tables, events queue, cursor, backfills and device settings.
  Nothing in `app_core` learns about more than one user; a core function
  still takes one `Deployment` and runs as the one user signed in on it.
  Multi-account is a switch in `app_tauri` that decides which file is
  open. No account column, no shared file.
- **A separate `device.db`** lists the accounts on the device. It holds
  no tokens and no user data; each account's token stays in that
  account's own file, in its `signed_in` setting, as today.
- **Sign out keeps the file.** A regular sign-out tells the server to
  forget the token and clears `signed_in` in the account's file, and
  the file stays. Signing back into that account reopens it and resumes
  from its cursor. Pending and conflicted changes stay in the file and
  push on the next sync, so a regular sign-out loses nothing. In
  `app_core`, `accounts::sign_out_on_device` stops wiping, and
  `sign_in_on_device` keeps the data when the user signing in is the
  file's user.
- **"Remove from device"** deletes an account's file and its row. It
  works on a signed-out account as well as the active one; on the
  active one it signs out first, and keeps the "changes would be lost"
  dialog, which the regular sign-out no longer needs.
- **The account menu lists every account on the device**, signed in or
  signed out.
- **At most 100 accounts** on a device, a number to raise later if ever
  needed.
- **Only the active account syncs.** A signed-in but inactive account
  makes no request, gets no pull and sends no pending change until it
  is made active again. No background sync, no badges, no notifications
  for inactive accounts.
- **Every file is migrated when the app starts**, inactive ones too, so
  a migration that fails shows up on the update, not weeks later on a
  switch. Start time grows with the number of accounts and their data,
  which is accepted.
- **`work_offline` stays in each account's file**, as today.
- **A switch opens the page the account was last on**, since
  2026-11-24. The router keeps each page loaded, its path and query,
  as the `last_visited_page` device setting in the active account's
  own file. The device's own pages, such as `/accounts` and sign-in,
  are not kept. An account with no page kept yet lands where a
  sign-in does. Signing in and starting the app always land on Home.
  That way, closing and reopening the app gets a person out of a page
  that does not load.
- **A signed-out account's file stays readable on disk** until it is
  removed, the same exposure the one signed-in account has today.
  Accepted; encrypting the files is a separate effort.
- **A sweep at start deletes orphans**: a row whose file is missing and
  a file that has no row, see "Orphans".
- **A switch waits for a sync in flight**, by taking the sync's own
  lock, see "The sync lock and a switch".
- **A 401 from a sync is a sign-out done elsewhere**: the token is
  cleared, the data kept, and the sign-in page shown with the email
  filled in, see "A token the server no longer knows".
- **One action, "Remove from device"**, in the account menu for any
  account, the active one included, always behind the same confirmation:
  any data stored on this device for that account is removed; signing
  in again later downloads what was synced.
- **No upgrade path for today's `app.db`.** Nothing is deployed; a dev
  deletes the old files with `bin/reset` and starts fresh.
- **Core writes into the file it is given.** The sign-in no longer
  wipes, and `sync_state::wipe()` goes, since "Remove from device"
  deletes the file instead. Core keeps no memory of users beyond the
  `signed_in` setting; which file a sign-in gets is the app's job
  alone, see "Two sections of the app".
- **The two halves of a sign-in are callable apart**: asking the server,
  then writing the answer into a file, so the app picks the file by the
  user id the server answered, see "Choosing the file at sign-in".
- **The app has two sections**, the device section that alone opens,
  closes and chooses files, and the account section that only ever
  gets the active account's deployment from it. Entering the device
  section closes the active account, so no account file is open while
  the sign-in page shows, see "Two sections of the app".
- **An accounts page in the app**, `/accounts`, lists the accounts on
  the device with Switch, Add account and Remove from device, and is
  where the sign-in and sign-up pages are reached from. The web never
  links it. Since 2026-11-25 a tap anywhere on an account opens it,
  except on Remove from device and its other buttons. The active account
  opens on its last visited page, read by `last_visited_page_get`, a
  signed-in one switches, and a logged-out one starts Log in.
- **A double tap on the avatar switches to the next account**, added
  2026-11-24: the next signed-in one in the order they were added to the
  device, back to the first after the last, `device_accounts_switch_next`.
  A signed-out account is skipped, since it needs the browser. With no
  other account signed in, nothing happens.
- **The web is single-account**, sign in and sign out, now and later.

## What `device.db` is, concretely

A SQLite file next to the account files, with one table:

| `device_accounts` | |
| --- | --- |
| `id` | made on the device, the primary key; names the account's folder, `accounts/<id>/`, since 2026-11-22, see `multi-server.md` |
| `server_id` | the server the account is on, unique with `user_id`; none for a local user |
| `user_id` | the user's id on the server |
| `email` | shown in the account menu |
| `signed_in` | whether the file holds a token, so the menu knows which accounts switch at once and which ask for the password |
| `last_active_at` | for ordering the menu and picking the account to open at start |
| `active` | the one account open now, at most one row |

It is what the account menu lists, what the app reads at start to know
which file to open, and what a sign-in consults to find an account's
existing file instead of making a new one. Nothing in it is secret,
and nothing in it is needed to read an account's file: a row is a
pointer to a file, no more.

## Proposed, to discuss

### The flows

- **At start.** Reconcile rows and files (see "Orphans"), migrate every
  file, then open the active account's file if it is signed in;
  otherwise show the sign-in page with the account menu available.
  With no rows, the sign-in page as today.
- **Sign in, with nobody active or as "Add account".** "Add account"
  deactivates the current account first, so nobody is active while the
  sign-in page shows. The server is asked, with the email and password,
  and answers the user and a token. Only then is a file chosen, by the
  user id in the answer: the account's existing file, or a new one. The
  token is written to that file's `signed_in`, the row is written or
  updated, and the account is activated. The account that was active
  stays signed in, inactive.
- **Sign up** is the same, with the server making the user first.
- **Switch.** A signed-in account switches at once: deactivate the
  current one, waiting for any sync in flight, activate the chosen one,
  write `active`, forget the memoized current user and navigate to the
  landing page. A signed-out account goes to the sign-in page with the
  email filled in.
- **Sign out** the active account: the server forgets the token,
  `signed_in` is cleared in the file and in the row, the file stays.
  Then the sign-in page, with the account menu available.
- **Remove from device**: sign out if signed in, delete the row, then
  the file.
- **Both work offline.** When the server cannot be reached, or answers
  an error, the device signs out all the same and logs that the server
  was not told. The token stays valid on the server until it expires,
  but the device held its only copy. A person can always sign out of,
  or remove, an account on their own device, even one whose server is
  gone. Telling the server later is in `../suggestions/accounts.md`.
- **The server answers 401.** See "A token the server no longer knows".

### A token the server no longer knows

The state: an account's file holds a token, and its row says signed in,
but the server has forgotten that token. That happens when the token
passed its year of validity, when the person signed out of that account
on the web or another device with a future "sign out everywhere", or
when the purge deleted it as expired. The device learns of it only on
its next request, which for an inactive account is the first sync after
it is switched to. The active account can hit it too, when its token
expires while in use.

Today `sync::run` reports the 401 as an error and the status shows it.
Proposal: the app treats a 401 from a sync as a sign-out done elsewhere.
It clears the token in the file and the row, keeps the data, and shows
the sign-in page with the email filled in; pending changes stay and push
after the next sign-in. `accounts::sign_out_on_device` already tolerates
the server answering 401 to the sign-out itself, so this is the regular
sign-out called from the sync wrapper in `tauri/src/sync.rs` when the
report carries `SyncError::Unauthorized`. This was deferred in
`../suggestions/accounts.md`; with accounts sitting signed in but
inactive for months it is needed from the start.

### Choosing the file at sign-in

Today the Tauri command `accounts_sign_in(email, password)` makes one
core call, `accounts::sign_in_on_device(deployment, transport, email,
password)`, which does two things in turn: it asks the server through
the transport, which answers the user and a token, and it writes that
answer into the deployment's file, the user's row and the `signed_in`
setting. There is one file, so the deployment is known before the call.

With a file per account, the second step has to go into that user's
file, `accounts/<user_id>/app.db`, and the user id is only known once the
first step has answered. So the Tauri command needs to run the first
step, then open or make the file, then run the second step. Core
already has the two steps as separate pieces, `transport.sign_in` and
the private `keep_signed_in`; the proposal is to make both callable
from the Tauri app, as `accounts::sign_in_on_server` and
`accounts::keep_signed_in`, and keep `sign_in_on_device` as the two in
one call for tests and the single-file case. No new concept in core,
and nothing about several users; one function becomes two.

The alternative is to pick the file before asking the server, by
looking the email up in `device_accounts`. That works until the email
can change: changed on the web, the row still has the old one, the
lookup misses, and the sign-in makes a second file for the same user
and resyncs from nothing. Not wrong, but a waste we can avoid.

### Two sections of the app

The app splits in two, by what each may touch:

- **The device section** owns `device.db` and the account files. It is
  the only code that opens, closes or chooses a file. Its commands are
  the account ones: list the accounts on the device, sign in, sign up,
  switch, sign out, remove from device. Its pages are the sign-in and
  sign-up pages and an accounts page listing the accounts on the device
  with Switch, Add account and Remove from device. It holds the active
  account, if any, as one state: active with its open `Deployment`, or
  nobody active.
- **The account section** is everything that exists today: posts,
  communities, sync, conflicts, and their pages. Its commands never
  take a `Deployment` from Tauri's managed state; each asks the device
  section for the active account's deployment and errs with "nobody is
  signed in" when there is none. It cannot name a file.

The slip described under "Choosing the file at sign-in", writing a new
account's token into the active account's file, becomes impossible to
write: the sign-in command has no active deployment in reach, only the
device section's own state, and the account commands have no file in
reach, only whatever the device section hands them.

Entering the device section closes the active account. "Add account"
from Bob's session deactivates Bob first, waiting for a sync in flight
as a switch does, so no account file is open while the sign-in page
shows, and Bob's file is never near Alice's sign-in. Bob stays signed
in, his token in his file, just not active. Cancelling goes back to the
accounts page, where Bob is one tap away; a switch is a file open,
which is a migration with nothing to do and the settings upgrade, so
there is no cost to closing and reopening. The start of the app is then
the same path as every switch: nobody active, read `device.db`, open
the last active account if it is signed in, else show the accounts
page. One code path for start, add account, sign-out and switch:
deactivate, then activate.

In the pages the split is the router guard that exists today, which
sends a visitor to the sign-in page before any page that needs someone
signed in. In the app it sends to the accounts page, which is where the
sign-in and sign-up pages are reached from. On the web the accounts
page does not exist: the nav never links it, and the sign-in page is
reached directly, as today.

The sync wrapper in `tauri/src/sync.rs` asks the device section for the
active account at the moment it takes its lock, and does nothing when
nobody is active, which covers the start, the sign-in page and the
moment between a deactivate and an activate. Only the device section
takes that lock to deactivate, so the sync in flight finishes against
the file it started on.

### Orphans

Two things must agree, a row and a file, and a crash can leave one
without the other:

- **A row without a file.** The sign-in writes the row and makes the
  file; killed in between, the row points nowhere. Also a person
  deleting files by hand.
- **A file without a row.** "Remove from device" deletes the row then
  the file; killed in between, the file stays. Also a `device.db` lost
  or reset while `accounts/` survives.

Proposal: a sweep at start, before the migrations. A row without a file
is deleted, since it is a half-finished sign-in and the account can be
added again; a file without a row is deleted, since nothing can reach
it and its token, if any, expires on its own. The sweep is a few file
checks and runs in no time. The alternative for a file without a row,
opening it to read its user and rebuild the row, recovers data after a
lost `device.db`, at the cost of opening unknown files; proposal: not
now.

### The sync lock and a switch

How a sync runs today: `SyncServer` holds a `tokio::sync::Mutex`, one
sync at a time; `sync::run` takes it, then `app_core::sync::run` pushes
the pending events in one request and applies the answers in one
transaction, pulls page by page with the cursor advanced in each page's
transaction, backfills communities just joined the same way, and prunes.
Every step is a transaction on the account's file, and the whole design
assumes a sync can die at any point: event ids make a push idempotent,
the cursor makes a pull resumable.

If a switch swapped the file without waiting, the running sync would
keep going against the old file, since it holds its own handle to it,
and finish normally. Nothing in the new account's file is touched. So
without a lock nothing corrupts and the next sync of either account
heals whatever was cut short. What goes wrong is smaller: the old
account keeps making requests after the switch, against the rule that
only the active account syncs; its `synced` event reaches the pages,
which would show the old account's report as the new one's; and
"Remove from device" on the old account would delete a file a sync
still writes to.

Proposal: the switch takes the same lock before swapping, so it waits
for a sync in flight to finish, and every sync reads the active account
at the moment it takes the lock, so the periodic and foreground syncs
after a swap run for the new account. The wait is one sync: usually well
under a second, at worst a first backfill of a large community, a few
seconds, during which the menu shows "Switching". No cancellation, since
a push cannot stop halfway.

### Where the code lives

- `tauri/src/device_accounts.rs`: the table, the sweep, the migrations
  of every file, the switch, the file naming, with tests on temporary
  directories. `app_tauri` has no tests today; this would be the first
  module with them.
- `tauri/migrations/device/`: the migrations for `device.db`, run
  through its own `sqlx::migrate!` path, separate from the account
  files' migrations in `core/migrations/sqlite/`.
- The managed `Deployment` goes. The device section's state is managed
  instead, holding the active account's `Deployment` behind a lock, or
  none; every account command takes a clone from it, `Deployment` being
  `Clone`, and runs as today.
- `DATABASE_PATH`, which dev uses to keep one file per worktree,
  becomes a data directory holding `device.db` and `accounts/`.

## The server's view

Nothing changes on the server: each account on a device holds a token
of its own, made by its own sign-in. A future "sign out everywhere"
deletes a user's tokens on every device, and each device then sees a
401 and marks that account signed out, data kept.

## Build order

Each step leaves every check green and the app working, so the work can
stop between any two.

1. **Core: sign-out keeps the data, sign-in in two halves.**
   `accounts::sign_out_on_device` clears `signed_in` and no longer
   wipes; `sign_in_on_device` no longer wipes; `sign_in_on_server` and
   `keep_signed_in` become public; `sync_state::wipe()` goes. A 401
   from a sync in the app signs the account out, data kept. Tests
   updated. The app still has one file, and a sign-out followed by a
   sign-in now resumes from the cursor, which is Option 3 on its own.
2. **The device section with one account.** `device.db`, its migration,
   `device_accounts.rs` with the sweep and the migration of every file,
   the managed state holding the active account or none, every account
   command taking its deployment from it, `DATABASE_PATH` becoming a
   directory, `bin/reset` knowing the new layout. Still one account at
   a time; the accounts page lists it. The first `app_tauri` tests.
3. **Several accounts.** Add account, switch, remove from device, the
   cap of 100, the accounts page complete with the confirmation, the
   nav entry to it, the "changes would be lost" dialog moved to remove
   from device. The two-device HTTP test gains a two-accounts-one-device
   case.
