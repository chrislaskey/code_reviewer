# Offline sync

How the desktop and mobile apps keep a SQLite file of their own, write to
it with the network off, and converge with the web server, the system of
record. The design is the one `phoenix/docs/design/offline-sync.md`
(in the history: `phoenix/` was removed after `f100d21`),
`offline-sync-client.md` and `multi-tenancy.md` settled, with the
reasons, kept here only as the decisions; what changes is how much code
it takes, since `core/` runs on the server and on every device.

## Decisions carried over

- **Conflicts are decided by versions, never clocks.** Every synced row
  has a `version` only the server increments. A change carries the
  version it started from.
- **A row changes only through events**, one per save, holding the fields
  the person changed, merged field by field on the server. A field someone
  else changed first keeps their value, and the device's value comes back
  as a conflict the person reapplies or lets go. Nothing is lost silently.
- **Soft deletes.** A delete is an update of `deleted_at`, so it syncs
  like any edit, and records who in `deleted_by_id`. Every list and get
  leaves deleted rows out. A purge removes them after 90 days.
- **Pull follows committed global sync versions.** The server commits each
  row's `global_sync_version` with the row and its `app_sync_changes`
  entry. The device stores the version it has pulled through for that
  feed. Row versions still decide conflicts. See
  [the Rust implementation](global-sync-version-build.md).
- **A row from the server never replaces a row with a queued change.**
  The push decides that row first.
- **A conflict is kept until the person resolves it.** Nothing is retried
  on its own.
- **Every function acts as a user**, who owns what they write and sees
  what is theirs or in a community of theirs. The server takes the user
  from the request's bearer token.
- **Inside a deployment, communities are the boundary.** A user joins
  communities; a post is in the communities its owner chose, zero or
  more, all equal; leaving takes the member's own posts along and
  rejoining within the window brings them back. The vocabulary and the
  reasons are in `phoenix/docs/design/multi-tenancy.md`.
- **Memberships and a post's community rows are decided by the server.**
  A device's event for one is what the person meant, applied through the
  same join, leave, add and remove the server runs, and the server's row
  comes back whatever the device wrote. There is no field merge for them.
- Ids are UUIDv7 from whichever device writes the row. Times are UTC to
  the microsecond.

## What is simpler than in Phoenix

- **The rules are written once.** The merge, the fields an event may
  change, a post's validation, who may change what and what a user sees
  live in `core/`, and a device's local write is the same `posts::create`
  the server runs. Phoenix's desktop client re-implemented every rule in
  Rust; here there is nothing to mirror.
- **One schema.** Every deployment runs the same migrations, so the
  events table and the sync columns are one migration everywhere, and no
  client schema is exported.
- **The web writes the way a device does, for free.** Every write in
  `core/` is an event. The server applies it at once; a device applies it
  at once too, and queues it for the next push. There is never a second,
  privileged write path, and there is no outbox beside the queue: a
  membership or a post's community row travels as an event like a post
  does, and only what the server does with it differs.
- **Sync is tested without HTTP.** The transport to the server is a trait
  with two implementations: HTTP for the apps, and one that calls the
  server's own functions in the same process for tests, resolving the
  token as the web server does. A test holds one server database and
  device databases for several users and syncs them in whatever order it
  likes, on SQLite and PostgreSQL both.

## Roles

The same code has to know whether it is the system of record or a copy,
so every core function takes a `Deployment`: the `Database` it opened and
a `Role`, `Server` for the web server and `Device` for the apps. Reads are
the same in both. A write differs in two places: the server increments
`version` and records the event in its log; a device leaves the version
as the server's and queues the event. Tests make both roles over
in-memory databases.

## Who the user is

Every core function takes the `users::User` acting. `app_users` is the
app's view of each person, their name and picture, one row per person,
written by the server at sign-up and synced like any row, so a post can
show who wrote it. Their email is their account's, `accounts_users`
(`core/src/data/accounts_users/`), a row with the same id that never
leaves the server: only `app_` tables sync, and `accounts_preferences`,
a person's own choices, see `multi-app.md` and `locales.md`.
Credentials never leave the server either: `accounts_credentials` holds the
password hash (`core/src/data/accounts_credentials/`) and `accounts_tokens` the API
tokens (`core/src/data/accounts_tokens/`), as hashes, good for a year, both by the
user's id. The flows are `core/src/accounts.rs`: `sign_up`, `sign_in`
and `sign_out` on the server, with an email and its emailed code, each a
plain server write and never an event; and `sign_up_on_device`,
`sign_in_on_device` and `sign_out_on_device`, which ask the server
through the sync `Transport`, so a test signs a device in without HTTP.

The server resolves every request's token to its user,
`users::get_by_token`, and answers one it does not know with 401: the
apps send the token as `Authorization: Bearer`, a browser the cookie a
sign-in set. A device keeps who signed in and their token as one device
setting, `signed_in`, null until a sign-in, and `users::signed_in` reads
it before every command; a sync with nobody signed in does nothing. A
sign-out tells the server to forget the token and clears the setting,
even when the server cannot be told; the data stays, pending and
conflicted changes included, so a sign-in by the same user resumes from
the cursor. Core writes into whatever
database it is given and never deletes one: which file a user signs
into, and when a file goes, is the app's business, a file per account
on a device, see `device-accounts.md`. A sign-in's two halves,
`accounts::sign_in_through` and `accounts::keep_signed_in`, are
callable apart for that.
There is no default user on any deployment.

## Columns on every synced table

| Column | Set by | Meaning |
| --- | --- | --- |
| `version` | the server only | 1 for a new row, incremented by the server on every write; 0 on a device's row not yet pushed |
| `global_sync_version` | the server only | the durable version of the row's last replicated change, null before a device create is accepted |
| `last_synced_at` | the server only | the time the server last wrote the row, used to age tombstones |
| `deleted_at` | whoever deleted | the soft delete, the time on that device's clock |
| `deleted_by_id` | the deployment applying the delete | the user who deleted, never sent by a device |

`updated_at` is the time of the last edit on the clock of whoever made
it, shown to people and deciding nothing. The server writes its own time
over a row it applies a change to, so a device's optimistic times are
replaced by the server's when its answer comes back.

The synced tables, parents first, which is the order a device writes
them in: `app_users`, `accounts_preferences` (a person's own choices,
to their devices alone, see `locales.md`), `app_communities`, `app_users_communities` (a
membership), `app_posts`, `app_posts_communities` (a post in a
community, under the owner's membership there), `app_files` (a file's
row; its bytes travel apart, see `offline-files-decisions.md`),
`app_files_communities` (a file in a community, the community's, see
`files-communities.md`). The list is `core/src/app/synced_tables.rs`,
each table implementing `SyncedTable` in its own module, which is how
sync, conflicts, purge and a local user's move reach every table
without naming one, see `template-and-apps.md`. Their foreign keys are
real on every deployment, with `on delete cascade`, which is what a
device's prune leans on.

## Events

An event is one change a person made, sent once and answered once:

```json
{
  "id": "0192f7c0-...",              // from the device, the idempotency key
  "table": "app_posts",
  "row_id": "0192f7a1-...",
  "type": "update",                  // or "create", with base_version 0
  "base_version": 3,                 // the row's version the change started from
  "base": {"body": "old"},           // the changed fields' values as the device saw them
  "changes": {"body": "new"},        // the values the person chose
  "made_at": "2026-11-08T10:00:00.000000Z"   // the device's clock, for ordering and display
}
```

`create` carries the fields the person set. `update` carries the fields
that changed, and `base` holds exactly those fields with the row's values
just before the save, so two saves made one after another on one device
never conflict with each other. A delete is an update with
`changes: {"deleted_at": ...}` and `base: {"deleted_at": null}`.

Which fields an event may change is the resource's rule, in its module.
Any other field is refused as `not_editable: <field>`:

| Table | Create | Update |
| --- | --- | --- |
| `app_posts` | `body` | `body`, `deleted_at` |
| `app_communities` | `name` | `name`, `deleted_at`, by a live member |
| `app_users_communities` | `user_id`, `community_id`, `role` | `deleted_at` |
| `app_posts_communities` | `post_id`, `community_id`, `users_communities_id` | `deleted_at` |
| `app_users` | nothing, the server makes a user | `name`, by the user themselves; `avatar_url`, by the server only for now, see `uploads.md` |
| `app_files` | `name`, `content_type`, `size`, `sha256`, `communities`, once the bytes are in the bucket | `name`, by a member of one of its communities |
| `app_files_communities` | `file_id`, `community_id`, by a member who sees the file | `deleted_at`, by a member of the community; the last one deleted deletes the file |

Who made the event is not on the wire: the server records the user its
token stood for, a device the user signed in, as `user_id` in
`app_events`. The owner of a post is the user who created it, and only
the owner's events change it; another user's are refused as `not_owner`
with the server's row.

### The merge, `events::merge`

For a post, given the row's current values at `version`, and the event's
`base_version`, `base` and `changes`:

1. At the server's version, every change applies.
2. Behind it, field by field: a field the server already holds the
   device's value for is skipped; a field whose server value is still the
   device's `base` value applies, since nobody else changed it; any other
   field was changed by someone else first. **Their change stays**, and the
   device's value is a conflict, `{"yours": ..., "server": ...}`.

The fields that applied are written in one update, which increments the
version once on the server. The answer is the event's status, `applied`,
`conflict` or `rejected`, the version after it, the conflicts, a reason
when rejected, and the row as it now stands on the server.

A community is merged as a post is, `name` field by field; only a live
member's events change it, another user's are refused as `not_a_member`
with the server's row.

A membership or a post's community row is not merged. The server reads
the event as what the person meant, a join, a leave, an add or a
removal, runs that against what it holds for the pair, which may be a
row under another id that a leave or a removal left behind and the
event revives, and answers `applied` with its row. A device applies its
own event the same way, so its row is right until the server's replaces
it; when the server's row has another id, the device's goes.

### The log

`app_events` is one table on both sides, created by the same migration,
and its rows never travel as rows, only as a push's events. On the server
it is the log: every event answered is recorded with its answer, in the
same transaction as the row, so a retried event gets the same answer and
nothing applies twice. On a device it is the queue: an event is `pending`
until the server answers it, deleted when applied, and kept as a
`conflict` until the person resolves it. A rejected create the server
sent no row for is refused: kept as a conflict with the reason, its row
kept on the device, and never sent again on its own. A rejected update
the server sent no row for stays pending with the reason in
`last_error`, unless the reason is that the row is unknown to the
server, see "Purge and resync".

**A refused create is never dropped.** The person decides, on the
conflicts page: Try again or Let it go, see "What the person must still
be able to see" in `docs/design/backend-protocol.md`. Until then, the
row's later events, and the creates of rows pointing at it by a foreign
key of `SyncedTable::foreign_keys` (a post's rows in communities, a
file's), wait with it: held back from each push, as a file's events are
while its bytes wait. A resync keeps the row. Letting it go removes the
row, its events and the rows waiting with it.

**A pending event is not tried for ever.** Each time the server
rejects it and it stays pending counts in `attempts`, and at 30,
`events::MAX_ATTEMPTS`, the event is refused: kept as a conflict with
the reason and nothing on the server's side, for the conflicts page.
Only the server's no to that event counts. No network, a timeout,
working offline, and a server or a bucket that is failing count
nothing, so an outage of any length refuses nothing.

A delete copied down, a post's community rows deleted with the post or a
member's with their membership, is written as a row, with its version
and sync time, and not logged as an event of its own: the event that
caused it is in the log, and a replay of it would copy down again.

### A write, in every role

`posts::update(deployment, user, id, body, community_ids)` reads the row,
checks the user owns it, builds the event with `base_version: row.version`,
`base: {"body": old}` and `changes: {"body": new}`, and applies it through
`posts::apply_event`, which is also what the server runs for a pushed
event. Apply checks the fields and the owner, merges against the row,
puts the applied fields onto the `Post` through its JSON form so the SQL
stays whole-row, validates, writes the row, and then does the role's
extra work: records the event and bumps the version on the server,
queues the event on a device. Then it makes the post's communities
exactly the ones given, an event per row added or removed. All of it is
one transaction, which is why a core function runs in one `Transaction`
and the SQL modules take a connection, never the pool.

Applying locally on a device always succeeds without conflict, since the
device's own row is the base. The conflict, if any, comes back from the
server.

A row made by a create keeps the time the person made it:
`inserted_at` is the event's `made_at`, the device's clock, unless that
is ahead of the clock of the deployment applying it, which then uses
its own (`Event::row_inserted_at`). So a post written offline on Monday
and pushed on Friday is Monday's.

A local user, someone using a device without signing up, writes rows
the same way and queues no event, `events::queue`, and their device
never syncs; if they move to an account, every row is queued as a
create then, see `accounts-local-and-trial-users.md`.

## Communities and what a user sees

A user sees their own posts, and the posts in the communities they are a
live member of; every community, so a device can list and join them; the
memberships in their communities and their own, live or not; and the
users who are live members of their communities. Each table's SQL module
holds that as one predicate, `visible_to!`, used by every read and by
the pull, so the pages and the sync agree, and the same SQL on a device
reads the subset the device holds.

Visibility changes without a row changing, which a cursor cannot see:
joining a community makes its old posts visible, leaving or a removal
takes them away. So, as in Phoenix:

- **Visibility is rows, and those rows sync.** A membership and a post's
  community row carry the sync columns like any table. Leaving is a soft
  delete of the membership, and of the member's own community rows there
  at the same moment, `deleted_at` copied once; rejoining revives the
  membership, the same row, and exactly the rows deleted at that moment,
  unless their post was deleted meanwhile, in which case the row takes
  the post's delete. A row the owner removed on purpose was deleted at
  another moment and stays removed. Deleting a post copies its delete
  onto its community rows. Deleting a community copies its delete onto
  every live membership in it, and from each onto its community rows,
  as a leave does; a deleted community cannot be joined or rejoined.
- **Every row a pull sends comes after the rows it points at.** A
  membership after its user and community, a post after its owner, a
  post's community row after the post, the community and the membership
  it was added under, and their parents in turn, each sent once. So a
  device with foreign keys writes a page in order, and a post that
  became visible through a new community row, however old, arrives with
  it. A row taken out of a community brings nothing along: its post may
  be one the user never saw, from before they joined, and a device
  without the post skips the row. `Page.cursor` is the version of the
  page's last row, not counting what was brought along. A parent
  brought along for another row does not advance it.
- **Joining is the device asking for everything in that community.** When
  a device applies a live membership of its own user that it did not
  have, or had left, it records the community in `app_sync_backfills` and,
  after the pull, asks `POST /api/sync/backfill` for every live post row
  in the community, paged, with the same rows brought along, until the
  last page, then forgets the community. A backfill cut short by the
  network is asked for again on the next sync.
- **Leaving and removal are one local prune.** After the pull and the
  backfills, a device deletes every post it does not own that is in no
  community its user is a live member of, and the rows under them go
  through the foreign keys. That covers a membership that ended, a post
  taken out of a community, a post deleted by its owner, and the post a
  removal brought along which the device never had.

The pull therefore answers "the rows this user sees that the server
wrote after the cursor, each after what it points at", not "rows changed
since the cursor".

## The endpoints

Three, on the web server, under the API, each for the user the bearer
token stands for:

```
POST /api/sync/push
  {"events": [<event>, ...]}
  -> {"events": [{"id", "status", "version", "conflicts", "reason", "server": {"table", "row"} | null}, ...]}

POST /api/sync/pull
  {"cursor": {"feed_id", "global_sync_version"} | null, "after": {...} | null, "limit": 200}
  -> {"rows": [{"table", "row"}, ...], "next": {...} | null, "cursor": {...} | null,
      "resync": false}

POST /api/sync/backfill
  {"community_id": "...", "after": {...} | null, "limit": 200}
  -> the same page, of the community's live post rows; 403 for a community the user is not in
```

A push applies the events in the order given, each in its own transaction.
A pull reads up to an upper bound, the feed's head when the first page
is read. `after` carries the previous page's `next` value and keeps that
bound. A page holds the rows the user sees, in version order: from each
table its first rows in the range, merged and cut at the limit. Other
users' changes take no room on a page. An intermediate page's cursor is
the version of its last row. The final page's cursor is the bound, even
when the last versions were someone else's.

A fresh device bootstraps from the current rows it may see. It saves its
cursor on the final page only. Until then it keeps the `next` of its last
page in `app_sync_state`, so a first pull cut short goes on from there on
the next sync. Backfill has its own continuation, kept on its row of
`app_sync_backfills` the same way, and leaves the main cursor alone. A
continuation the server no longer answers, from another feed or below
its floor, gets `resync`.

Cursor versions and row global sync versions are decimal strings on the wire.
Requests carry the sync protocol version in `x-protocol-version`. Responses
carry the settings version in `x-settings-version`. See [server
selection](multi-server.md).

## One sync on a device, `sync::run`

As the user signed in, with the device's token:

0. **Ask** the server which waiting files' bytes its bucket has. The
   events of a file whose bytes it has not are held back from the push.
1. **Push** every event pending as the push begins, in `made_at, id`
   order, 500 a request, `sync::PUSH_BATCH`, or fewer when their JSON
   would pass four fifths of `sync::MAX_PUSH_BYTES`, 5 MB, the most
   the server takes in a push; one event at least. A request over
   that is refused whole and would be sent again unchanged at every
   sync, so a device never makes one. Each request's answers are
   applied before the next is sent, so a push cut short has kept what
   was answered. Apply the answers:
   `applied` deletes the event; `conflict` keeps it with the conflicts;
   a `rejected` with a server row deletes the event, the server wins; a
   rejected `create` without one is refused and kept for the person,
   with its row, and the later events in the request of that row and of
   rows pointing at it stay pending, waiting with it; a file's create
   the server only lacks the bytes of counts a try instead; a rejected
   update without one for a
   row the server does not know deletes the device's copy and keeps the
   change as a conflict with nothing on the other side; any other
   `rejected` without one keeps the event pending with the reason. Then,
   for each row the push answered, once no event for it is still pending,
   apply the server's row from its last answer, parents first, which
   brings the merged values and the new version; a server row under
   another id than the device's replaces the device's row. An event
   queued while the push was in flight is still pending, so its row is
   left alone until the next push.
2. **Pull** pages from the stored cursor until `next` is empty, upserting
   each row unless it has a pending event, and storing `cursor` after each
   page. A page that says `resync` starts over, see below.
3. **Backfill** each community in `app_sync_backfills`, page by page.
4. **Prune** the posts the user no longer sees.
5. **Upload** the bytes of the files held back, then push their events.
   Last, so nothing else waits on bytes.
6. **Clean** the account's folder of the bytes no file points at.

Steps 0, 5 and 6 are `offline-files-decisions.md`.

A push that fails as a whole changes nothing on the device, and the pull
still runs. One sync runs at a time. A sync runs when the app starts,
when it comes to the foreground, after every local write, from the Sync
now button, and every half minute.

The device keeps its cursor in `app_sync_state`, one row, which the server has
too and leaves empty, since every deployment has the same tables.

### How a sync is started and run

A sync is a task, not a job in a queue. The app (`tauri/src/sync.rs`)
starts one on five triggers: when the app starts, when it comes to the
foreground, after every local write, from the Sync now button, and from
a timer every half minute. Foreground is the window gaining focus on the
desktop and the app resuming on a phone, which Tauri reports as window
events; since a resume is followed by a focus, and a click back into a
desktop window is not news, a foreground sync is skipped when one
started within the last five seconds. A
write's command awaits the row going into the device's file, then spawns
the sync on the app's async runtime without awaiting it and returns, so
the page navigates while the sync runs and the sync finishes whenever it
finishes. The server never starts one: there is no push from the server
and no websocket, and another device sees a change on its own next pull.

**Two syncs never overlap.** The task takes a Tokio mutex before running.
A second task that arrives while one is running waits on the lock, then
runs a full sync of its own. At most one spawned task waits: a trigger
that finds one already waiting spawns none, since the waiting one reads
the data when it runs. Nothing is lost by that: the only queue is the
data, the pending events in `app_events` and the communities in
`app_sync_backfills`. Three writes in a row during a sync spawn one more;
5,000 files created in a row spawn a few, not 5,000. The Sync now button
and the timer run their own sync either way. A sync that left files for
the next, `Report::more_files_to_send`, spawns the next as it ends.

**The steps follow each other in the same task.** `sync::run` is one
async function: it pushes the pending events, keeps a push failure in
the report, then pulls regardless, backfills and prunes. A push that
fails as a whole changes nothing on the device. A pull that fails ends
the sync with the error. Either way the task ends with one `synced`
event to the pages, carrying the report or the error, which the nav
shows and the pages reload on. There are no callbacks between the steps.

Offline, every trigger still spawns a sync, which fails at the push and
the pull within the request timeout and reports the error; the events
stay pending and the next trigger tries again. A timer only runs while
the app is in front: a phone suspends the app in the background, so
nothing syncs until it comes back, and the foreground trigger is what
syncs it then.

### Working offline

A person can choose to work offline, and choose to go online again, from
the nav. The choice is a device setting, a row of `device_settings`, the
table of what a person chooses on a device, one JSON value per key so a
new setting needs no migration. Every setting is declared once, with the
value a device starts with, in `core/src/data/device_settings/`, and the app
brings a device up to date with that list when it starts, adding what is
missing and leaving what the person set; that is also where a release
that changes a setting rewrites what an earlier one stored. The choice
holds across restarts, and it is a decision, not a guess: while it is
set, every trigger still runs a sync, but the sync reads the choice first
and returns at once with a report marked offline, making no request.
Writes keep queueing as they always do, the nav shows "Working offline"
with the count waiting, and going online runs a sync at once that pushes
them. The web has no such choice: it is the server.

Real loss of the network is not detected. A sync tries and fails within
the request timeout, the nav shows the error, and the next trigger tries
again. Detecting it would only guess at what the attempt finds out for
certain, and the switch covers the cases where no attempt may be made:
a metered connection, a demonstration, a test of the offline path.

## Conflicts

The nav shows the pending count and "N changes were not saved on the
server", a link to the conflicts page, only inside the apps; the web has
nothing to sync. The page lists each conflicted event, newest first: the
post by its first line, when the change was made, and for each field the
person's value and the value now on the device, which is the server's as
of the last sync. **Reapply** queues a new update from the row's current
version with the person's values and syncs; it applies unless someone
changed the field yet again. **Let it go** deletes the event. A conflict
on a post that is gone can only be let go. A create the server refused
is there too, by what it is about with the reason, a post, a file or a
post's row in a community: **Try again** queues its create as it was,
**Let it go** removes the row from the device, with the rows waiting
with it, and a file's bytes.

## Purge and resync

Scheduled purge removes a committed prefix of change entries older than
the retention window. It records the last discarded global sync version as
the minimum valid cursor and removes eligible tombstones with no surviving
synced children. The purge's wall time still determines which entries and
tombstones are old enough to remove.

A cursor below that minimum or from another feed gets an empty page marked
`resync`. The device drops its cursor and backfills, removes replicated rows
it can replace, and bootstraps current authorized rows. It retains pending
changes, their rows and required parents, the current user and unsent files.
Its memberships queue the community backfills again. Its own pending events are pushed before the
pull, as always, so a resync normally finds none; those a failed push
left behind are kept, with the rows they are about, since a pending
event carries its own change: the next push answers it and the server's
row replaces the device's. Conflicts are kept too; they belong to the
person. A change to a row the server has purged, which it answers as
unknown, is kept as a conflict with nothing on the other side and the
device's copy of the row goes, so the conflicts page shows it as a post
that is gone, to let go. That settles the question the Phoenix design
left open, what a resync does with queued events: nothing, except push
them.

## Build order

Each phase ends in a state that works, is tested and is worth keeping.

| Phase | Builds | Proves |
| --- | --- | --- |
| 1 | the columns, `app_events`, `app_sync_state`, the role, every write through `events` | the web behaves as before; versions climb on the server and stay on a device; a deleted post is hidden; a retried event gets the same answer |
| 2 | `sync::push` and `sync::pull` in core and on the server, the in-process transport, the tests | two devices converge; the same field edited on both keeps the first and returns the other as a conflict; different fields both apply; two offline edits on one device do not conflict with each other; a pulled row never replaces one with a pending event |
| 3 | the device: queueing, `sync::run`, the HTTP transport, the timer, the status in the nav | a post written offline reaches the web and the other device |
| 4 | the conflicts page, Reapply and Let it go | a conflict reapplied reaches both devices; one let go sends nothing |
| 5 | `app_users`, `accounts_tokens`, the owner on a post and `deleted_by_id`, the user on every function, the bearer token, the dev user | a user sees only their posts; another user's change is refused with the server's row; a token the server does not know is 401 |
| 6 | communities, memberships, a post's community rows, visibility, rows brought along, backfill, prune, the pages | a post is seen in the communities its owner chose and nowhere else; leaving takes the member's posts along and rejoining brings back exactly what left; joining on the web fills a device and leaving empties it; a device's own join pushes and backfills; a row the server holds under another id replaces the device's |
| 7 | the purge, the horizon, the resync | what was deleted before the horizon goes with its rows and events; a device away too long starts over and matches; a pending change survives a resync; a change to a purged row is a conflict to let go |

Deferred, roughly in the order they will come: the rest of the login
flow, magic links, confirmation, password change and reset, see
`docs/suggestions/accounts.md`; deleting an account; a community's own
page and its members; roles beyond `member`; the "something changed"
nudge; a stale-form check on the web, which needs the page to send the
version it loaded. See `docs/suggestions/`.
