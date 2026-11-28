# Review: global sync versions as the foundation for other backends

**Review of `cr-otherbackends` at `5861a22` ("v2 sync protocol"), 2026-11-27.**
§3.2, §3.3 and §3.4, and part of §3.1, are fixed in step 0 of
[the backend seam](../design/backend-seam.md#as-built-2026-11-27).
It covers the plan (`docs/design/global-sync-version-build.md`), the two
Elixir proposals in `docs/suggestions/elixir-workflow-*.md`, and the Rust
code that carries out the plan. File paths below are paths on that branch.
The Commanded design itself is out of scope here. It is the next analysis.

Evidence gathered: I read the plan, the diff of all 82 files, and the new
tests. `cargo test -p app_core --all-targets` passes on the branch: 188
tests, on SQLite only. `TEST_POSTGRES_URL` was not set, so none of the
PostgreSQL paths ran: the row lock, the repeatable-read snapshot, and
the concurrent-writer test. I did not run the axum or tauri suites,
clippy, or `cargo audit`.

## 1. Short answer

**The product model does not change.** The way apps are built does
change: there is a new write discipline on the server.

What stays the same for a person using an app, and for the shape of
an app:

- Offline-first devices with their own SQLite database. A write is an
  event queued with the row in one transaction.
- The web server is the system of record. Push answers each event with
  `applied`, `conflict` or `rejected`.
- Row `version`, `base_version` and the field-by-field merge still
  decide conflicts. Conflicts are still kept until the person resolves them.
- Soft deletes, a 90-day purge, and a resync that keeps pending work.
- Community visibility: join → backfill, leave → prune, parents sent
  before children.
- Local users never sync. Trial users stay web-only.
- The Svelte pages are untouched. Nothing in `svelte/src` reads the cursor.

What changes is how a device finds out what changed, and what every
server write must do so that it can:

| Before (protocol 1) | After (protocol 2) |
| --- | --- |
| The cursor is `(last_synced_at, id)`, the server's wall clock, with a one-minute overlap | The cursor is `(feed_id, global_sync_version)`, a counter committed in the write's own transaction |
| A server write sets `last_synced_at` | A server write takes the feed lock, increments the counter, inserts an `app_sync_changes` entry, and stamps the row |
| A write that took longer than the overlap could be skipped forever | A reader can never pass an uncommitted change (on both databases, by construction) |
| A server database reset or restore went unnoticed by devices | A new `feed_id`, or a cursor above the head, makes the device resync |
| An edit to the same value still bumped `version` | An edit to the same value is a no-op: no version, no feed entry |

So the foundation is **sound in its core property**. Commit-ordered
versions are exactly what a second writer, such as an Elixir projector,
needs. The cost is real, though, and some of it should be fixed before
apps copy the pattern. The four issues that matter most:

1. **Every synced write on PostgreSQL is serialized through one row lock.**
   Sign-in is too (§3.1).
2. **Incremental pull pages count other users' changes.** So a pull's
   round trips grow with the whole server's write volume, not with what
   this user can see (§3.2).
3. **A first sync (bootstrap) cannot resume.** An interrupted first sync
   starts again from zero (§3.3).
4. **Publishing a change is manual, repeated code in each table.** An app
   table that forgets it fails silently (§3.4).

None of these needs a schema rethink. All four can be fixed while a reset
is still free.

## 2. Is this the right foundation?

Yes, with two design questions to settle first.

**Why it is right.** A multi-writer feed needs exactly one thing from
the cursor. Once a device saves N, nothing may later commit at or below
N. Wall-clock time can't promise that. Neither can a bare sequence, or
"the highest event Commanded has accepted". The branch holds the
counter's row lock from the time it takes a version until commit. So
versions commit in the order they were taken. A reader's snapshot
sees a head that has nothing uncommitted beneath it. That is the right
contract, and the plan states it clearly. Later, an Elixir projector can
write its rows, stamp them, and insert its change entries in its own
PostgreSQL transaction under the same contract. Rust then keeps the
entire read path: visibility, parents, backfill, prune and bootstrap.
That split is good. Elixir never has to reimplement who sees what.

**Design question A: what is `app_sync_changes` for?** Today the pull
does not resolve change entries into rows. It reads the versions only
to shrink the range (`scan_page`, `core/src/sync.rs:394`). Then each table
is scanned by its own `global_sync_version` within that range, under its
visibility rule. Soft deletes are rows, so tombstones need no entry. A
hard-deleted row's entry is silently skipped. So the log does exactly
two jobs:

- it bounds how much work one page examines;
- it sets the floor (`app_sync_changes::purge`).

The floor could be derived without the log: it is the highest stamp
among the tombstones purged. Then the log would cost one insert per
write and a 90-day table for little gain. There are two coherent
choices:

- **Keep the log and make it carry its weight.** It would record hard
  removals ("row X of table T is gone"). That gives a deliverable path
  for removals outside the purge, such as cascades or account deletion.
  It would also be the single thing an Elixir projector must write. A
  later audience key on the entry (owner or community) could let a pull
  read only relevant entries.
- **Drop the log.** Set the floor from the purged tombstones. Page by
  visible rows the way bootstrap already does. This makes the write path
  simpler: lock, increment, stamp.

I recommend keeping the log, for the Elixir future. But it should get
the removal entries, and it should not drive the page bound (§3.2).

**Design question B: is a global lock acceptable?** On SQLite the lock
costs nothing, because SQLite already has one writer. On PostgreSQL it
turns the server into a single-writer system for all replicated data.
A Commanded projector would also queue behind Rust writes. For the
template's scale this is acceptable, and it is the simplest correct
design. Two things would make it a deliberate choice rather than an
accident:

- narrow what holds the lock (§3.1);
- write down the alternative to move to when contention shows up. That
  is the commit watermark pattern: a plain sequence, plus a reader
  bound of "the lowest version still held by an open transaction". It
  is tracked with an `xid8` column and `pg_snapshot_xmin`. It lets
  writers run in parallel and keeps the same cursor contract. It is
  PostgreSQL-only, and harder to get right.

## 3. Findings

### 3.1 The feed lock is held too broadly (PostgreSQL)

`app_sync_versions::lock` is `SELECT … FOR UPDATE` on the singleton
(`core/src/data/app_sync_versions.rs:57`). `lock_for_role` takes it at
the top of every table's `apply_in`. That is before the duplicate check,
the ownership check, validation and the merge. It is held until commit.
Locking first is deliberate: it gives one lock order, counter then
rows, and avoids deadlocks. But it also means:

- Duplicate pushes, refusals and no-op edits serialize with every other
  writer, though they publish nothing.
- `accounts::sign_in` locks (`core/src/accounts.rs:195`) even for an
  existing account. That path only inserts a sign-in token and writes no
  synced row. Only the sign-up branch creates an `app_users` row. So
  every sign-in code request on the server contends with every sync push.
  `sign_in_from_trial` (`:253`) and `confirm` (`:553`) do the same.
- One push request of 500 events is 500 sequential locked transactions.
  This is fine, but it bounds push throughput for the whole server.

**Recommendation.** Take the lock only on paths that will publish. Where
the decision needs the current row, lock that row first
(`SELECT … FOR UPDATE` on the row). Take the counter lock last, just
before `next()`. The order would then always be rows → counter. Writers
that publish take rows in a fixed order (parents before children, by
id). For the account paths, take the lock in the branch that creates or
changes an `app_users` row, not at the start of the transaction. Add a
PostgreSQL test with many concurrent writers that checks for deadlocks
and duplicate versions. The retry on `40001/40P01` in `sync::push` is a
good safety net. It does not replace a tested lock order.

### 3.2 Pull cost grows with server-wide writes, not with the user's

The incremental page reads `limit + 1` versions from `app_sync_changes`
(`core/src/sync.rs:404`), whoever they belong to. It then shrinks
`range.through` to the 200th one. Picture a device away for a day on a
server where others wrote 200,000 replicated changes. It makes about
1,000 round trips, nearly all of them empty pages. Protocol 1 paged by
visible rows, so it needed about one. This is the plan's stated choice:
"bound the number of entries examined, including entries the actor
cannot see". It is the right bound on *server work*, but the wrong unit
for *round trips*.

**Recommendation.** Page incremental pulls the way bootstrap already
does. Read each table's visible rows in `(after, H]`, ordered by stamp,
`limit + 1` each. Merge them, and truncate to `limit`. Bound the server
work separately with a much larger cap on entries examined per request,
say 10,000–50,000. When that cap hits, return an empty page with a
continuation. The rule "a page may be empty yet advance the cursor" is
kept for the rare case. The common case becomes one round trip again.
With the index on `global_sync_version`, the per-table query is a range
scan plus the visibility join, as before.

### 3.3 Bootstrap is not resumable

Bootstrap pages return no cursor until the last page. That is on
purpose, so an interrupted bootstrap "restarts safely". The device also
saves no continuation (`pull_pages`, `core/src/sync.rs:812`). So a phone
whose first sync of a large account keeps getting cut off, by
backgrounding or a weak network, starts again at version 0 each time.
Protocol 1 saved its cursor per page, so a first sync resumed.

**Recommendation.** Save the bootstrap continuation in `app_sync_state`
(a nullable `pending_continuation`), in the same transaction as each
page's rows. On the next sync, send it as `after`. The server already
checks feed and floor through `covered_by`, and answers `resync` if it
is stale. Clear it when the final page saves the cursor. Do the same for
a community backfill. Its queue entry already survives, but it starts
again from 0.

### 3.4 Publishing is manual and duplicated across tables

Each table's write path calls `lock_for_role`, then either
`record_sync_change` or an inline `if let Some(version) =
app_sync_changes::record(..)`. That makes eight tables, plus the trial
and account paths. The changelog asks every app to do the same for its
own tables. If one write path forgets to publish, the row is written
with a stale stamp, and devices never see the change. Nothing reports
it. If one forgets the early lock, PostgreSQL can deadlock under load.

**Recommendation.** Make publishing impossible to forget:

- Move the stamp into the data layer. The `pub(crate)` server-role
  `create`/`update` of each `data/app_*` context would take the role (or a
  `Publication` value only `app_sync_changes::record` can make). They
  would stamp the row themselves. `Accepted*` already gates validation the
  same way.
- Add a framework invariant test that runs after the sync suites. It
  would check that every server row of every registered table has a
  stamp, and that the stamp equals the latest `app_sync_changes` entry for
  that row (or is at or below the floor). It would also check that no
  two rows share a stamp. Then an app table that skips publishing fails
  CI, not production.
- Provide a `SyncedTable` test kit an app can run against its own table:
  create, edit, delete and revive on the server, then pull on a device.

### 3.5 Smaller findings

- **Membership "joined" now fires on any newer version**
  (`core/src/app/memberships.rs:558`, `incoming.version > had.version`).
  A role change, or any edit to a live membership the device already
  has, queues a full community backfill. It is harmless for correctness
  but costly. Only a revival (deleted → live) or a first confirmation
  should backfill. If the clause exists to catch a missed leave/rejoin,
  compare `deleted_at` transitions or `inserted_at` instead.
- **Two server stamps per row.** `last_synced_at` stays, for tombstone
  aging, next to `global_sync_version`. Consider renaming it
  `server_written_at` while a reset is free. Its old name says it is the
  sync cursor, which is no longer true.
- **No-op edits.** Same-value edits no longer bump `version` or
  `updated_at`. That is right for the feed. But it is a visible change:
  a page that shows "edited at" won't move when someone saves without
  changing anything. Note it in the changelog.
- **Hard deletes outside purge.** Account deletion hard-deletes
  `accounts_preferences`. The trial sweep deletes trial rows. Unconfirmed
  sign-ups delete `app_users`. Today no live replica holds those rows:
  the device loses its token, trials are web-only, and unconfirmed users
  were never pulled. So it is safe. But the rule should be written down:
  "a hard delete of a synced row is allowed only when no device can hold
  it". The removal entries of §2 A are the general fix.
- **Unsigned continuations.** A client can forge `after`. Visibility and
  membership are rechecked on every page, so nothing leaks. The worst
  case is a device that skips data it was owed. Signing the continuation
  is optional hardening.
- **A quiet server forces resyncs.** If nothing is written for 90 days,
  the floor moves to the head. Every device whose cursor is below the
  head then bootstraps. This is correct, and it matches protocol 1's
  horizon rule. It's just worth knowing.
- **Purge holds the feed lock** while it deletes tombstones across all
  tables. On a large server that is a pause for every writer. Batch it,
  or take the lock only for the floor update and the tombstone delete
  per table.
- **PostgreSQL coverage.** The tests for the main property (writer
  order, rollback, snapshot) only ran on SQLite in this review. Run the
  suite with `TEST_POSTGRES_URL` before merging. The PostgreSQL lock and
  `REPEATABLE READ READ ONLY` snapshot are where this design is most
  likely to surprise.

## 4. Impact on apps built from the template

**New apps.** An app's synced table now needs a few more things:

- a nullable `global_sync_version INTEGER/BIGINT CHECK (> 0)` column and
  its index;
- the field on the struct, with `serde(with = "sync::version::optional")`;
- `foreign_keys()` declared on its `SyncedTable` (a test checks this
  against the schema);
- `VersionRange` in place of `Cursor` in `list_synced_after` and
  `list_live_in_community_after`;
- on every server write path: take the lock first, then record the
  change and stamp the row. That includes bulk SQL, which must stamp each
  row with its own version, as `trial_users::touch_communities_of` now
  does.

Everything else about building a feature is unchanged: the event, the
field merge, the page, and the Tauri/Axum mirror. §3.4 would shrink the
list to "declare the column and the foreign keys".

**Existing apps (Code Reviewer, Emoji Status).** These steps are
mechanical, but there are a lot of them:

- edit the baseline migrations;
- run `bin/reset`, and wipe devices and simulators;
- install builds that match the server. Protocol-1 apps get HTTP 426.

The app's own synced tables need the same edits as above. Watch for any
direct SQL that sets `last_synced_at` or bumps `version`. Grep for both.
Each one is a silent sync bug until it publishes. The changelog entry
covers this. It should also name the no-op-edit change and the
lock-ordering rule.

**Operations.** The server database gains one change row per replicated
write, kept for 90 days. A restored backup must get a new `feed_id`
before it serves clients. The plan says so, but no tooling does it yet.
A `bin/` command or a startup check would make it hard to forget.

## 5. Issues that remain, before and after

These existed under protocol 1. They are unchanged by protocol 2, and an
Elixir backend will inherit them:

- **Visibility changes are not feed changes.** Gaining access needs a
  separate backfill. Losing it needs a device-side prune. A new audience
  rule (roles, private posts) needs both halves written by hand.
- **No in-between status.** An answer is applied, conflict or rejected.
  There is no "accepted, still processing". Long workflows need a synced
  status row or a new answer kind, which is a protocol bump.
- **Retry evidence expires.** `app_events` on the server is purged with
  the rows. A device offline for more than 90 days with a pending edit
  gets a fresh decision, not the original answer.
- **Device clocks.** `made_at` and `deleted_at` still come from the
  device's clock. They are informational, but they are shown to people.
- **One system of record.** The server database is the only authority.
  Rust on SQLite remains a single-writer server. With Elixir it becomes
  PostgreSQL-only, as the proposal accepts.
- **The plan cache** in `device_settings` stays outside sync, refreshed
  online.

## 6. Recommended next steps, in order

Updated 2026-11-27. The next steps are now in
[the backend seam](../design/backend-seam.md#7-next-steps).

1. **Done.** Narrow the feed lock. A sign-in code request, and a retried
   event, no longer take it. The suite ran on PostgreSQL. Narrowing it
   further, with row locks, is still open.
2. **Done.** Page incremental pulls by visible rows.
3. **Done.** Persist bootstrap and backfill continuations.
4. **Done.** Move stamping into the data layer, and add the feed
   invariant test.
5. **Decided, 2026-11-27.** `app_sync_changes` stays as it is, with no
   removal entries (§2 A). The rule instead: a hard delete of a synced
   row only when no device can hold it, in `offline-sync.md`.
6. **Decided, 2026-11-27.** The membership backfill trigger stays, since
   memberships will grow. Renaming `last_synced_at` is still open
   (§3.5). The purge now holds the feed lock only to move the floor.
7. **Started.** The Commanded backend, in `core_elixir/`. Its projector
   writes the rows, their versions and the answers in one transaction,
   under the feed lock.
