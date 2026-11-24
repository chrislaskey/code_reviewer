# Offline-first writing and syncing

How mobile and desktop clients write locally first and reconcile with the
server's database. Companion to `multi-tenancy.md`, which settles that a
tenant needing isolation is its own deployment, with one database on the
server and one file per server on the device.

Decisions already made that this doc builds on:

- **Conflicts are decided by versions, never clocks.** Every synced row
  has an MVCC `version` that only the server increments. A client sends
  back the version its change started from. See "Leak 2".
- **A user-editable resource changes only through events**, one per save,
  merged field by field. A field someone else changed first keeps their
  value, and the device's value comes back as a conflict the person can
  reapply or let go; nothing is lost silently. Whether the server also
  keeps a log of every event is decided per resource: posts do, profiles
  do not. The log is the foundation for event sourcing later, deriving a
  row from its events. See "Events".
- **Soft deletes** everywhere, with `deleted_at` and `deleted_by_id`, and a
  separate job that purges rows deleted more than **90 days** ago. A client
  away longer than that does a full resync.
- **One replication path.** No whole-database mode for private deployments.
  If every member is in every community, the normal visibility rule already
  sends everything.
- Clients generate ids, so primary keys are **UUIDv7**.
- Sync is **per deployment**. A sync session talks to one server.
- **Ecto migrations are the schema source of truth.** Client SQLite
  migrations are generated from them. See "Schema source of truth."
- **Datetimes are UTC ISO 8601 text with microsecond precision**, exactly as
  the SQLite adapter writes them. See "Datetime format."
- **Sync runs over HTTP** on the server with the existing API token as a
  bearer token. The websocket channel later carries only a "something
  changed" nudge. See "Transport."

## The baseline design

What has worked before, stated precisely so the weak points are visible:

1. Every row in every synced table carries a server-assigned timestamp
   recording when the server last wrote it.
2. Each client keeps one cursor per server: the server timestamp of its last
   successful pull.
3. **Pull.** Client sends its cursor. Server returns every row, across all
   tables, with a server timestamp greater than the cursor. Client upserts
   them. Rows with `deleted_at` set are applied as deletes, or kept and
   filtered, depending on whether the client wants undo.
4. **Push.** Client sends every row it changed locally since its last push.
   Server upserts each one, keeping the row whose `updated_at` is later.
5. Deletes are updates that set `deleted_at`, so they flow through the same
   path. A purge job hard-deletes rows past the retention window.

This is a good design. It is small, it needs no extra tables, it is easy to
debug by reading rows, and LWW makes push idempotent. The rest of this doc is
about the four places where it leaks and the cheap fixes for each.

## Columns on every synced table

The same columns exist on the server and on every client, so one table
definition serves both and the sync code is the same shape everywhere.

| Column | Who sets it | Meaning |
| --- | --- | --- |
| `version` | the server only | The MVCC version, 1 for a new row, incremented by the server on every write. Finds concurrent edits. A client copies it and sends it back as the version its change started from. A row created on a device and not yet pushed has 0. |
| `updated_at` | the client that made the edit | The time the edit was made, from that client's clock, guarded by the server. Shown to people; decides nothing. |
| `last_synced_at` | the server only | The time the server last wrote the row, from the server's clock. The pull cursor. |
| `deleted_at` | the client that deleted | Soft delete. Same clock and guard as `updated_at`. |
| `deleted_by_id` | the client that deleted | Who deleted it. |

The web app counts as a client whose clock happens to be the server's clock.
A LiveView edit writes `updated_at` and `last_synced_at` with the same value.

On a client, `last_synced_at` is copied verbatim from the server and never
written by the client. After a local edit it is simply stale until the next
push response brings the new value. Which rows need pushing is tracked in a
separate outbox table, described under "Push, in detail," so the column means
the same thing on both sides and can be declared not null on the server.

On the server, `last_synced_at` is set inside the write transaction from the
database clock, and any value a client sends for it is ignored.

`server_seq`, described under leak 1, is an optional integer alternative to
`last_synced_at` as the cursor. It is not needed to start.

## Where the baseline leaks

### Leak 1: a timestamp is not a safe cursor

A row's server timestamp is assigned when the write happens, not when it
commits. With concurrent writers, a transaction can start at T1, a client can
pull with cursor T2, and the first transaction can commit at T3 with a
timestamp of T1. The client advances its cursor to T2 and never sees that row.

On SQLite this cannot happen today because there is one writer at a time. It
can happen the day the server moves to Postgres, and it is the kind of
bug that shows up as "one user is missing one post" months later.

Two workable fixes:

- **Timestamp plus buffer.** Keep the server timestamp, assigned by the
  database clock and never by an app server, and pull from
  `cursor - buffer` where the buffer is at least the longest transaction
  timeout, typically 60 seconds. Upserts are idempotent so re-applying rows
  is harmless. Pagination must order by `(synced_at, id)` and carry both in
  the cursor, because many rows can share one timestamp and a page boundary
  inside a tie would skip rows.
- **Integer sequence.** A single counter row in the database. Every write
  to a synced row bumps the counter in the same transaction and stores the
  new value in the row's `server_seq`:

  ```sql
  UPDATE sync_counter SET value = value + 1 RETURNING value;  -- 90417
  UPDATE app_posts SET ..., server_seq = 90417 WHERE id = ?;
  ```

  Pull is `where server_seq > ? order by server_seq limit ?` and the next
  cursor is the last row's value. No clock, no ties, exact paging. On SQLite
  the single writer makes it exact with no buffer. On Postgres a plain
  sequence has the same commit-order gap as a timestamp, so either keep the
  buffer or make the counter a locked row, which serializes writers.

  The counter grows once per server write, not per client or per sync, and
  it is per deployment. As a 64-bit integer it holds about 9.2 quintillion
  values, so one million writes per second would take roughly 290,000 years
  to exhaust it. SQLite integers are always 64-bit. Postgres needs the
  column declared `bigint`, which in an Ecto migration means `:bigint`, not
  `:integer`, since `:integer` maps to a 32-bit column there and would
  overflow at 2.1 billion.

Both work. The sequence is tidier to paginate and immune to server clock
steps; the timestamp is readable, already familiar, and is the same
`last_synced_at` column the clients carry. Either way keep the overlap.

Recommendation: `last_synced_at` plus buffer as the cursor, ordered by
`(last_synced_at, id)`. Add `server_seq` only if the server moves to
Postgres and the buffer proves awkward. The rest of this doc says
`last_synced_at` for the cursor; `server_seq` substitutes everywhere.

### Leak 2: whose clock decides a conflict

The baseline decided conflicts by last write wins on `updated_at`, the
time a device said it made the edit. Device clocks drift and can be set
on purpose, and an edit made offline on Monday and synced on Friday
carries Monday no matter what happened on the server in between. A
server-side bound stops a clock reading 2031 from winning forever, but a
slow clock still loses edits silently, and nothing tells the user. Hybrid
logical clocks fix the ordering between always-connected servers, but a
device's offline edit still cannot have seen the edits it missed, and a
device with a fast clock can push every later write forward.

So conflicts are decided by **versions**, MVCC, and no clock is trusted:

- Every synced row carries a `version`. The server sets 1 on insert and
  increments it on every write, inside the write transaction. In Phoenix
  `App.Data.Context` does this for every update of a stored row with
  `Ecto.Changeset.optimistic_lock/3`, so a server-side write from a stale
  copy, such as a form loaded before someone else saved, fails with a
  `:version` error instead of overwriting.
- A client never changes a version. It copies the server's with the row,
  and a change it pushes carries the version it started from.
- **User-editable resources** change through events, merged field by
  field, below.
- **Server-wins rows**, app users, memberships and a post's community
  rows, are pushed whole and applied through their flows, and the
  server's row always comes back. A community is created from a pushed
  row and never changed by one.

`updated_at` is guarded by the server as in the baseline and only shows
people when something was edited.

The pull cursor is a separate question, Leak 1, and stays
`last_synced_at`. `server_seq` is still the way to drop timestamps there
too.

### Events

Every resource a user edits, posts and profiles today, changes only
through events: a device sends what the person changed rather than the
whole row. The row is still what pulls send and what every read uses.

An event is one change a person made, sent once and answered once:

```json
{
  "id": "0192f7c0-...",             // generated by the device
  "table": "app_posts",
  "row_id": "0192f7a1-...",
  "type": "update",                 // or "create", with base_version 0
  "base_version": 3,                // the row's version the change started from
  "base": {"body": "old"},          // the changed fields' values as the device saw them
  "changes": {"body": "new"}        // the values the person chose
}
```

One event per save, holding every field the save changed, not one per
field. The resource's flow applies it, `App.Data.Posting.apply_event/2`
or `App.Data.Profiles.apply_event/2`, merged by `App.Data.Sync.Merge`:

1. **Already recorded** under this id, for a resource with an event log:
   the same answer again, nothing applied twice.
2. **At the server's version**: every change applies.
3. **Behind it**, field by field: a field whose server value is still
   the device's `base` value applies, since nobody else changed it. A
   field the server already holds the device's value for is skipped. Any
   other field was changed by someone else after the device read it. **The
   change that reached the server first stays**, and the device's value is
   a conflict.
4. The fields that applied are written in one update, which increments
   the version once. With an event log, the event is recorded in the same
   transaction with what applied, the conflicts, and the version after
   it, even when nothing applied.

Each event is answered with its status, `applied`, `conflict` or
`rejected`, the version after it, the conflicts as
`{"body": {"yours": ..., "server": ...}}`, and the row as it now stands.

A worked example. A post at version 3 has a body and a `deleted_at`. The
phone, offline, edits the body. The laptop, online, also edits the body
and the server is at version 4. The phone syncs:

- An event changing only `deleted_at`, deleting the post, applies, since
  `deleted_at` is still the phone's base value. Version 5, nothing lost.
- An event changing the body conflicts: the laptop's body stays, and the
  answer carries the phone's body as `yours`. The phone keeps the
  conflict and shows "1 change was not saved on the server". The person
  can **reapply** it, which is a new event from version 4 with the
  laptop's body as its base, so it applies unless someone changed the
  body yet again, or **let it go**.

Why `base` values and not only the version: a device that edits the same
post twice offline sends two events from the same version. The second
must not conflict with the first. Comparing against `base`, which for the
second event is the first event's value, handles this without the server
knowing which device sent what. The cost, a field changed away and back
again counts as unchanged, is the right answer for a person anyway.

The web app saves the same way: a form sends the values it loaded as the
`base` and the fields the person changed, and shows a conflicting field
inline with Reapply and Let it go, see `App.Web.ProfileLive`.

**The event log.** A resource opts in to `app_events`, one table shared
by every resource that logs, server-only and never synced. Each row's
events go in a stream, its post's for a post, its links, tags on it and
community rows, recorded in the same transaction as the row and gone
with the stream's row when the purge hard-deletes it. See "The event
log" in `related-resources.md`. It is the history, the record of
what was not applied, and what makes a retried event exact. Profiles keep
none: a retry is still safe, since a field already holding the value is
skipped, except that a retry arriving after someone else changed the same
field shows a needless conflict. A log is the foundation for event
sourcing later, deriving the row from its events; today the row is
written directly and the log sits beside it.

Adding a user-editable resource is: an `apply_event/2` in its flow,
usually a spec for `App.Data.EventFlow` with the fields an event may
change, an entry in `@event_tables` in `App.Data.Sync`, and, for a log,
`event_stream/1` on its schema. The web saves through the same path with
`App.Data.Sync.save/4`.
A row pushed for one of these tables is refused with `events_only`. A
device keeps its events until the server answers them, and the
conflicted ones until the person resolves them, see
`offline-sync-client.md`.

### Leak 3: soft deletes and the purge horizon

Soft deletes are the right call. Deletes sync as updates, undo is possible,
and `deleted_by_id` gives an audit trail for free. Three consequences to
handle:

- **Every query filters `deleted_at is null`.** Put it in the data context so
  callers cannot forget. The server already routes everything through
  `App.Data.Context`, which is the place.
- **Unique constraints collide with deleted rows.** A deleted slug blocks the
  same slug being reused. Use partial unique indexes,
  `where deleted_at is null`, which SQLite and Postgres both support.
- **A client offline longer than the retention window misses deletes.** The
  purge removed the tombstone before the client pulled it. The server must
  record its purge horizon, the oldest `last_synced_at` still guaranteed to be
  intact. A client whose cursor is older than the horizon gets told to drop
  its local file and pull fresh. This is also the recovery path for any
  corruption, so it is worth building early.

Purge has to respect foreign keys: a deleted post's community rows,
links and tags on it go with it. A delete copies its `deleted_at` onto
the rows below, so they age out together, see "Deletes" in
`related-resources.md`. Purging in dependency order, oldest first, in small batches, is
enough.

### Leak 4: visibility is not a row update

The pull in the baseline sends rows that changed. In a public deployment a
device must not pull the whole database, only the rows its user can see: owned by
them, or in a community they belong to. Visibility changes without the row
changing:

- User joins a community. Thousands of old rows become visible, none of them
  with a recent `last_synced_at`.
- User leaves a community, or is removed. Rows should disappear from the
  device.
- A post is removed from community C. Members of C who cannot see it another
  way should lose it.

A worked example. Alice's phone has pulled everything visible to her up to
seq 90,000. On Tuesday she joins the Gardening community, which has had
thousands of posts added to it over two years. Her next pull asks for
visible rows above 90,000. Those posts have seqs in the low thousands, so
none arrive. The cursor is blind to her visibility expanding. The same
blindness appears at the row level: an owner adds an existing post to
Gardening, the new `app_posts_communities` row arrives because it is new, but the
post it points at is old and does not.

The fix has three parts, all driven by the client so the server never has to
remember what any device holds:

- **Visibility is rows, and those rows sync.** `app_users_communities` and
  `app_posts_communities` carry the same `last_synced_at` and soft-delete columns as
  everything else. Removing a post from a community is a soft delete of its
  `app_posts_communities` row. Leaving is a soft delete of an `app_users_communities`
  row. Both arrive in the delta like any other change.
- **An `app_posts_communities` row carries its subtree.** Whenever a pull returns
  one, the server also includes the post it points at and the rows that hang
  off it, comments and reactions, regardless of their `last_synced_at`.
  Because comments reference the `app_posts_communities` row rather than the
  post, the subtree is well defined. Re-sending a post the client already
  has is a harmless upsert. This handles "existing post added to my
  community" with no extra state.
- **Joining is the client asking for every post in that community.** When
  the client applies a new `app_users_communities` row for its own user, it calls
  a backfill endpoint for that community. The endpoint returns every live
  `app_posts_communities` row for the community with the same carrying rule,
  paged. The delta pull and the backfill use one server function: "these
  `app_posts_communities` rows, plus what they point at."
- **Leaving and removal are one local cleanup.** After applying any batch,
  the client deletes every local post it did not owner that no longer has a
  live `app_posts_communities` row for a community it belongs to. Comments and
  reactions go with their `app_posts_communities` row through the client's own
  `on delete cascade` foreign keys. One reachability pass covers a removed
  membership, a post removed from a community, and a deleted community. A
  removed membership also arrives with the member's own `app_posts_communities`
  rows already soft-deleted by the server, since leaving cascades through the
  membership on the server side. A revive, such as rejoining within the
  retention window, is the reverse: the membership and `app_posts_communities`
  rows arrive with `deleted_at` cleared, and the carrying rule brings back
  the posts and comments the client had pruned.

The pull endpoint therefore answers "rows visible to user U with
`last_synced_at` later than C, plus the subtree of every `app_posts_communities`
row in that set,"
not "rows changed since C." That query is the heart of the server side and
deserves its own tests.

## Push, in detail

The baseline sends dirty rows. Details that matter:

- **Tracking what to push.** A client-side outbox table of pointers:

  ```sql
  sync_outbox  (table_name, row_id, queued_at, attempts, last_error)
               unique (table_name, row_id)
  ```

  Every local write upserts one entry for the row, refreshing `queued_at`.
  A push reads the outbox, loads each row's current state, and sends it.
  On success it deletes entries whose `queued_at` is earlier than the time
  the push started, so a row edited while the push was in flight stays
  queued for the next one. A rejected row keeps its entry, with the attempt
  count and the server's reason, which is also what a pending-changes count
  or a debugging screen reads from.

  This is for server-wins rows, where only the row's current state matters.
  It stores no payload and no history. One entry per row, three small
  columns. User-editable resources queue events instead, see
  `offline-sync-client.md`.
- **Dependency order.** Parents before children: communities before posts,
  posts before `app_posts_communities` rows and comments. Either push in table order or have the
  server apply the whole batch in one transaction with deferred foreign key
  checks. One transaction per batch is the better default: a batch is
  accepted or rejected as a unit.
- **Authorization on every row.** The server validates each pushed row
  against the user's memberships and ownership. A client cannot add a post
  to a community it is not in, or edit a post it does not own. Rejected rows
  come back in the response with the server's authoritative version, and the
  client overwrites its local copy. This means **permission rows are
  server-wins**: `app_posts_communities` rows, memberships, and roles are never accepted
  on a matching version alone.
- **Idempotency.** A retried row gets the server's row back, its own
  edit; a retried event gets its recorded answer. Neither applies twice.
- **Attachments.** Binary data does not go through row sync. The client
  uploads the file to object storage first, gets a key, and the row carries
  the key. A row referencing an unuploaded file is not pushed.

## Client bookkeeping

In the file on the device:

```sql
sync_state   (last_pulled_at, last_pulled_id, server_purge_horizon)
sync_outbox  (table_name, row_id, queued_at, attempts, last_error)
sync_events  (id, table_name, row_id, type, base_version, base, changes,
              made_at, status, conflicts, attempts, last_error)
```

`last_pulled_at` and `last_pulled_id` together are the cursor: the
`last_synced_at` and `id` of the last row received. `sync_outbox` is the set
of rows with local changes not yet accepted by the server, and
`sync_events` the edits, pending or in conflict.
Every synced table carries the same sync columns on both sides, listed under "Columns on
every synced table" above, and nothing device-only. The profile on the
device, the user's id and email, is a cached copy with no sync cursor.

Realtime is optional. Polling on an interval or on app foreground is enough
to start. A Phoenix channel can later send a "something changed"
nudge that triggers a pull, which is cheaper and simpler than pushing the
rows themselves over the channel.

## Schema source of truth

Ecto migrations define every table once. They are adapter-neutral, so the
same migration runs on SQLite today and on Postgres if the server moves,
with no second file. Clients are SQLite forever, so they need SQLite
migrations and nothing else.

A mix task in the style of `mix app.svelte` produces them. For each
migration it runs the migration against a scratch SQLite database with
`log_migrations_sql` on, captures the statements the adapter executed, and
writes them to `priv/client_migrations/<version>_<name>.sql`. Clients apply
the files in version order and record which they have applied, exactly as
Ecto does with `schema_migrations`. A test fails when a migration has no
up-to-date file, and the generated files are committed.

Three rules keep the export clean:

- **Only synced tables are exported.** A schema is marked as synced, the
  way a component is marked `@universal_component true`, and the task includes only
  migrations that touch marked tables. Server-only tables such as
  `users`, `users_tokens`, `app_purges` and `sync_counter` never reach a
  device.
- **Client-only tables are hand-written on the client.** `sync_state`,
  `sync_outbox` and `sync_events` are not Ecto schemas. Each client platform owns one small
  migration for them.
- **Column types are whatever the SQLite adapter emits.** Binary ids are
  `TEXT`, datetimes are `TEXT`, booleans are `INTEGER`. The clients do not
  get to choose, which is the point.

The alternative, hand-written raw SQL migrations consumed by both Ecto and
the clients, would need one file per dialect once Postgres arrives, with the
Postgres file untested until then, and would give up the adapter neutrality
Ecto already provides. Rejected.

## Datetime format

Datetimes written by different platforms are compared, by the pull cursor,
and the field merge of `deleted_at`, so the format has to be identical
everywhere. Phoenix leads: the SQLite
adapter writes UTC datetimes as ISO 8601 text, and that text is the format.

```
2026-10-25T14:03:07.412906Z
```

The trap is precision. Ecto's `:utc_datetime` writes no fractional seconds
while `:utc_datetime_usec` writes six digits, and the two sort wrongly as
strings: `...07Z` is greater than `...07.412906Z` because `Z` sorts after
`.`. So:

- Every sync column is `:utc_datetime_usec`, no exceptions. The exported
  client SQL inherits the format.
- The push endpoint parses every incoming datetime and rejects a row whose
  values are not UTC with exactly six fractional digits. The server is the
  gatekeeper, and a client with a wrong format finds out on its first push
  rather than losing a conflict silently later.
- Clients store server-provided values verbatim and write their own in the
  same shape, so every comparison on a device is between values in one
  format.

Integer microseconds since the epoch would sidestep the string-sorting
problem entirely at the cost of a custom Ecto type and unreadable values in
a SQL shell. Not needed while the rules above hold.

## Transport

Push, pull, and backfill are three HTTP endpoints on the server:

```
POST https://<host>/sync/push
POST https://<host>/sync/pull
POST https://<host>/sync/backfill/<community id>
```

The existing API token, the one the websocket connects with, goes in an
`Authorization: Bearer` header. A plug resolves the user from the token
before anything else runs. Bodies are JSON, compressed, with the cursor in the body and a page
size the server caps.

HTTP rather than the websocket because sync has two needs the channel serves
badly. Backfills are large and want compression, resumable pages, and
testing with `curl`. Background sync on mobile only works over HTTP, since
iOS and Android run background transfers for URL sessions and not for open
sockets. The channel keeps its current request-response job and later gains
a single message, "something changed," that tells a connected client to
pull. Until then, clients pull on foreground and on a timer.

## Alternatives

### A. Baseline with the fixes above

Server-assigned sequence cursor, LWW on a server-bounded client edit time,
inline soft deletes with a purge horizon, visibility as synced rows with
client-driven backfill and pruning.

Pros: smallest surface area. Plain tables, plain upserts, readable by a
human in a SQL shell. Works with SQLite or Postgres on the server. No
dependency on an engine's roadmap. Fits Ecto and `App.Data.Context` without
contortion.

Cons: you own every edge case. Visibility backfill and the purge horizon are
easy to get subtly wrong. Row-level LWW loses concurrent field edits. No
history.

### B. Append-only change log

Instead of marking each row, the server appends every write to a `changes`
table (table, row id, operation, payload, seq). Clients pull the log and
apply it. Rows themselves carry nothing sync-specific.

Pros: full history and audit for free. Cursor semantics are exact because
the log is the thing being paginated. Replay can rebuild any client or any
derived table. Easier to add new merge policies later.

Cons: a second copy of every write, with its own retention policy. Visibility
filtering now applies to log entries, which is harder than filtering rows.
Compaction is needed or the log grows forever. More to build than A, and the
history is often never used.

Worth choosing only if history or undo across devices is a product
requirement.

### C. CRDT-backed tables

Libraries such as cr-sqlite add column-level clocks and merge rules to
ordinary SQLite tables on both ends. Two databases merge in either direction
with no conflicts by construction.

Pros: column-level merge, no lost edits, peer-to-peer capable, device clocks
do not matter. Partial sync is a solved problem within the library.

Cons: per-column metadata roughly doubles storage. The merge model is foreign
to Ecto and to a relational mindset, and debugging a bad merge means
understanding the clock vectors. Foreign-key-like invariants are not what
CRDTs are good at. With a single clear owner per resource the conflict
problem they solve mostly does not arise.

### D. Sync engines

Hosted or self-hosted engines that watch the database and replicate subsets
to clients. The relevant ones at time of writing: ElectricSQL and its Phoenix
integration `phoenix_sync`, PowerSync, and Rocicorp's Replicache and Zero.
Writes generally still go through your own API; the engine handles the read
path and partial replication rules. Verify current state before choosing;
this category changes quickly, and at least one well-known product in it was
discontinued in 2024.

Pros: partial replication, backfill, and cursor correctness are the engine's
problem. Realtime included. Often a client SDK for each platform.

Cons: nearly all of them replicate from Postgres, not SQLite, which forces
the server to Postgres. One more moving part per deployment, and the
deployment-per-tenant model multiplies it. Their partial replication
rules have to express "visible through membership and presence," which is
possible but is the first thing to prototype. Vendor and license risk.

### Comparison

| | A. Baseline+ | B. Change log | C. CRDT | D. Engine |
| --- | --- | --- | --- | --- |
| Server store | SQLite or Postgres | either | SQLite | Postgres |
| Conflict model | row LWW | pluggable | column merge | usually LWW via API |
| Lost edits | concurrent field edits | per policy | none | per policy |
| History | no | yes | no | no |
| Partial sync | hand-built | hand-built, harder | library | built in |
| Fits Ecto and Data.Context | yes | yes | awkward | partly |
| Ops per deployment | none extra | log retention | none | engine instance |
| Debuggability | high | high | low | medium |

## Recommendation

Alternative A. The baseline design with four refinements:

1. `last_synced_at` as the cursor, set only by the server inside the write
   transaction, pulled with an overlap buffer and ordered by
   `(last_synced_at, id)`. Clients copy it from the server and never write
   it. A client-side outbox of row pointers tracks what needs pushing.
2. MVCC versions decide conflicts, never a clock. User-editable
   resources change through events merged field by field, and a field
   someone else changed first comes back as a conflict for the person.
   Permission rows are server-wins.
3. Inline soft deletes with partial unique indexes, a purge job, and a
   recorded purge horizon that forces a full resync for stale clients.
4. Visibility as synced rows. `app_users_communities` and `app_posts_communities`
   sync like any table. An `app_posts_communities` row carries its subtree, joining triggers a
   client-driven backfill, and the client prunes unreachable rows after every
   batch.

Revisit if any of these become true: history or undo across devices is
required (B), many people edit the same rows concurrently (C), or the server
moves to Postgres anyway and a sync engine's partial replication can
express the visibility rule (D).

## Guiding questions

Still open. Answered ones have moved to the decisions at the top.

- **Which resources need an event log?** History, audit and exact
  retries come with one. Posts have one, profiles do not.
- **Is undo or edit history a product feature?** If yes, alternative B earns
  its cost. Not at the start.
- **How big is a user's visible subset of a public deployment?** Determines
  whether initial sync is seconds or minutes, and whether backfill needs
  paging and progress UI.
- **When does the "something changed" nudge arrive?** Polling on foreground
  and a timer is the start. A chat-like feature is what forces the channel
  message.
- **When does the server move to Postgres?** Expected before any app's
  public deployment has enough writers to feel SQLite's single writer. When
  it happens, the cursor's overlap buffer becomes mandatory rather than
  free.
