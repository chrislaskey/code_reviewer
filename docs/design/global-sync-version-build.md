# Implement global sync versions in Rust core

**Implementation notes for the template.** Replace timestamp-based discovery with a server-assigned `global_sync_version` on SQLite and PostgreSQL. Ship the server, device schema and sync protocol together. There are no deployed apps to migrate. Edit the original migrations and require a hard development reset downstream. The [offline sync design](offline-sync.md) describes the running protocol.

## Contract to build

After a device saves global sync version 52, every change it was entitled to see through 52 has been accounted for. A later commit that affects its replica must be discoverable after 52. The server can advance a device through versions that belong to other users without disclosing those rows. A later grant of access must bring in newly visible older data.

| Value | Owner | Purpose |
| --- | --- | --- |
| Request `id` | Originating writer | Recognize a retry and recover the push answer. |
| Row `version` and event `base_version` | Server for the row, device for the edit's base | Decide concurrent edits to one row. |
| `global_sync_version` | One server database | Order committed changes across all replicated resources in this deployment. The device persists its progress in its active account database. |
| Sync protocol version | Server and client builds | Refuse an incompatible peer before exchanging sync data. |

For example, editing a post at row version 3 can produce row version 4 and global sync version 52. Sharing it with a community can publish a separate link at 53 in the same transaction. A push response can reveal version 53 to one device before that device has pulled 52. Only a completed pull page can advance its saved pull cursor.

## 1. Define the baseline schema and Rust types

Update both migration trees together. The sync machinery starts in `20261120000011_app_sync_create.sql`. The replicated resources have their own earlier or later create migrations. Include the SQL used to install the test-only notes table. Use the repository’s table-owned contexts with private SQLite and PostgreSQL adapters. [SQLite baseline](../../core/migrations/sqlite) · [PostgreSQL baseline](../../core/migrations/postgres) · [Existing cursor context](../../core/src/data/app_sync_state/mod.rs) · [Test table](../../core/src/sync/test_notes.rs)

| Storage | Proposed fields and rules |
| --- | --- |
| New `app_sync_versions` | Singleton keyed by a fixed ID. Store a feed UUID, the last committed `global_sync_version` (starts at 0), and the minimum valid incremental cursor (starts at 0). Initialize once per server database. A fresh database gets a new feed UUID. |
| New `app_sync_changes` | One entry per committed change, keyed by positive `global_sync_version`, with registered table name, row UUID and server write time for retention. The entry identifies the changed row. It contains no credential, row payload, client patch or user-specific list of recipients. |
| Each registered synced row | Replace `last_synced_at` as a cursor with nullable `global_sync_version`. Index the new field for bootstrap and backfill. Server-originated rows have a value. A device-only create has none. Preserve the last confirmed value on an optimistic edit. Keep business timestamps and server write time wherever needed for retention. |
| `app_sync_state` | Replace the time/UUID cursor with feed UUID and last applied global sync version. An absent cursor requests a fresh bootstrap. Each active account already has its own file. |
| `app_events`, `app_purges`, `app_sync_backfills` | Keep the request log, conflict records, purge audit and pending community list in their existing roles. Record the effective version floor with the purge run. Use the server singleton as the current floor. |

Represent versions with a nonnegative checked value backed by Rust `i64`, PostgreSQL `BIGINT` and SQLite `INTEGER`. Specify the JSON representation once in shared serde types. Use decimal strings for cursor and row global sync versions at JSON boundaries so a JavaScript round-trip preserves values above 2^53. Test that round-trip. Business row versions keep their existing contract. The two new infrastructure tables are private to sync and stay outside the synced table registry.

## 2. Assign the version in the authoritative write transaction

Give the new contexts a narrow transaction-scoped operation shared by all server writers. For each material change, increment the singleton using `UPDATE … SET global_sync_version = global_sync_version + 1 … RETURNING global_sync_version`, stamp the row, and insert the change entry with that version. The caller owns the commit. Complete external I/O before taking the counter lock. PostgreSQL replicated event handlers lock the counter before reading the current row for the decision. That also gives purge and multi-row writers a common lock order. Keep that part of the transaction short.

On PostgreSQL, concurrent increments of the singleton serialize. On SQLite, a write transaction has one writer. If A obtains 52 and stalls, B cannot publish 53 until A commits. If A rolls back, its row, change entry and increment all roll back. A plain sequence or a separately committed counter would allow the phone to skip a late commit. Retry an aborted transaction at the operation boundary with the same request identity. Keep operations with unknown commit outcomes eligible for existing request deduplication. [Transaction ownership](low-level-contexts.md) · [PostgreSQL locking](https://www.postgresql.org/docs/current/explicit-locking.html#LOCKING-ROWS) · [SQLite transactions](https://www.sqlite.org/lang_transaction.html)

Use one version per affected row, including dependent rows updated by the same operation. Acquire the counter once for a range if an existing bulk operation already knows the affected IDs. Give them a deterministic order. Commit every entry and row together. Once the transaction holds the counter lock, later statements in that transaction can allocate further versions safely.

### Inventory the writers before swapping the reader

| Write path | Required behavior |
| --- | --- |
| Ordinary server writes, from Axum or pushed events | Publish on a first create, applied edit, soft delete or revival. A partial field merge publishes the actual changed row once. A recorded refusal, unchanged row or duplicate request consumes no global sync version. Keep the existing transaction around the row and its push answer. |
| Community and file/post relations | Publish the membership and all changed link rows. Cover community delete, leave/rejoin, revivals, post deletion, and file removal from its last community. Preserve relationships and shared deletion times used to decide which links to revive. |
| Signup, account deletion, guest conversion and maintenance | Publish the profile or other replicated consequences. In particular, trial conversion bulk-updates communities to make them discoverable. Enumerate and version every affected community. Audit hard deletes and database cascades. If a live replica might retain the row, supply a deliverable removal or invalidate affected cursors before removing it. |
| Direct writes and caches | A server-side direct write that changes a synced view or visibility uses the same operation. A credential or provider receipt with no replicated consequence gets no change entry. `accounts_preferences` does sync. The separately refreshed plan cache in `device_settings` keeps its existing lifecycle. |
| Device writes and replication | Offline edits, local-only use, optimistic reapply, imported rows, parent expansion and pruning allocate no server versions. The local-to-account move queues creates the server can accept later. |

Walk every table in `app::synced_tables::list`, plus the test-only table. Trace both SQL adapters, the bulk maintenance contexts and any direct SQL assigning `last_synced_at` or incrementing row versions. Today a single hook in `events::record` would miss trial-conversion SQL and confuse an answered request with a changed row. Also replace the membership backfill check that currently recognizes an unconfirmed device row by its null sync timestamp. [Registry](../../core/src/app/synced_tables.rs) · [Event recording](../../core/src/sync/events.rs) · [Trial SQL](../../core/src/data/trial_users/sqlite.rs) · [Membership application](../../core/src/app/memberships.rs)

## 3. Replace pull and backfill together

Keep the existing endpoints, authentication, page size bounds and protocol header. Bump `PROTOCOL_VERSION` from 1 to 2. Change the shared request/response types used by the Axum handlers and by both Rust transports. Update device status serialization and the Axum JSON assertions. Remove the timestamp overlap, timestamp/UUID sorting and the end-of-pull call to `now()`. [Core protocol and device loop](../../core/src/sync.rs) · [Table contract](../../core/src/sync/synced_table.rs) · [Transports](../../core/src/services/web_server.rs) · [Axum handlers](../../axum/src/sync.rs)

**Incremental pull.** On the first page, read the committed upper bound H, feed UUID and floor from the same database snapshot used to produce that page. On continuations, keep the original H. Validate the feed, floor and bounds on every request. Use a consistent snapshot within each response, including permission checks, change lookup, rows, parents and purge state. In PostgreSQL, ordinary READ COMMITTED can use different snapshots for successive statements. Use a read-only repeatable-read transaction for the page or an equivalent single-snapshot query. Each page request opens a fresh transaction.

1. Scan `app_sync_changes` in ascending version over `after < global_sync_version <= H`. Bound the number of entries examined, including entries the actor cannot see. Read one extra entry, or perform an existence query, to distinguish a full last page from a page with more work.
2. Resolve entries through the registered resource with its existing visibility predicate, including its rules for tombstones and link removals. Load the current row only when its stamp matches the scanned change. A superseded row will have a later entry, possibly beyond H. A missing row is safe to skip only under the removal/floor rules. Batch lookups by table where practical.
3. Keep the current parent expansion, parent-first apply order and per-page deduplication. Supply only parents authorized by the existing sync rules. A link removal can travel without fetching a newly inaccessible parent. A parent may already be newer than H. It can satisfy a dependency but cannot advance the cursor.
4. Return the **last examined** version on an intermediate page and H on the final page. A page with zero visible rows can have a continuation. Save the rows and cursor atomically on the device. A crash before commit replays the page. A crash after commit can start a new bounded pull from that saved cursor. Guard against an older delivered row overwriting a newer confirmed row. Preserve the pending-edit guard.

For example, from saved cursor 51 with H = 55 and scan limit 2, inaccessible entries 52 and 53 produce an empty page with cursor 53 and a continuation bounded at 55. A post at 54 can bring its owner as a parent. The final cursor can reach 55 even if entry 55 was private. A commit at 56 waits for the next pull. Rust-only and future projection writers have the same obligation to publish a discoverable change.

Use a cursor containing `feed_id` and `global_sync_version` for incremental progress. Have the server issue a separate continuation containing the scan kind (incremental, bootstrap or community backfill), feed ID, last examined version and upper bound. Scope a backfill continuation to its community. Preserve the current transport convention of a `next` value for another page and an explicit `resync` response. Reject malformed continuations, decreasing ranges and cross-kind reuse. An absent cursor and absent continuation start bootstrap. On bootstrap pages return a durable cursor only on the final page. Backfill pages return only their own continuation.

**Bootstrap and full resync.** A fresh account must receive visible live state even when its original change entries have expired. Enumerate the registered tables directly with the same visibility and dependency rules as pull. Use a bounded scan of rows stamped through H, ordered by global sync version, with a separate bootstrap continuation. Fetch `limit + 1` candidates as needed, including when all rows come from one table. If a row is updated past H between pages, its new change is picked up by the following incremental pull. Complete the baseline only after the last page. Keep the durable cursor absent until then, so an interrupted bootstrap restarts safely. Retain existing resync cleanup, including pending edits, their required parents, the current user and unsent file bytes. If purge overtakes H during bootstrap, restart with a fresh baseline. Commit the final page and baseline cursor together.

**Community backfill.** Capture its own bound, list live rows in the community and reuse parent expansion. Recheck live membership on every page. A join exposes older content even if it is below the device’s global cursor. A backfill continuation must never advance the account-wide cursor. If interrupted, keep the community queued and restart idempotently. If membership has been revoked, retire the obsolete backfill, run the ordinary pull/prune path and verify the community’s private data disappears. Preserve the existing visibility rules for posts, files and memberships. [Current offline visibility rules](offline-sync.md#communities-and-what-a-user-sees)

## 4. Retention, resets and continued offline use

Keep the current time-based retention policy for physical cleanup. Use time to decide when data is old enough to purge. Use the integer floor to decide whether a cursor can still catch up. Under the same serialization rule as writers, prune only a committed prefix of the change log, record its greatest discarded version as the floor, and commit both together. If entries have nonmonotonic wall times, stop at the first entry still inside the retention window. A cursor equal to the floor can pull the next version. A cursor below it must bootstrap.

Keep soft-deleted rows long enough to deliver their removals. Existing purge requires both the deletion and the last server write to be old. Preserve that behavior for a recently modified tombstone. Before removing any replicated tombstone, ensure its final change is at or below the new floor. Account for dependent rows erased by cascades, including paths outside scheduled purge. Recheck the floor across paged pulls. Exercise purge and publish concurrently. [Purge workflow](../../core/src/sync/purge.rs) · [Existing deletion predicate](../../core/src/data/app_purges/sqlite.rs)

Bind cursors to the feed UUID and the selected account/server. A foreign UUID, a stale cursor below the floor, or a saved version above the server head triggers full resync. If a backup restore rewinds a feed, rotate its identity before serving clients so an old cursor cannot silently match a different history. Normal protocol-2 resync must preserve pending user work. The one-time development reset deliberately discards it.

## 5. Verification and delivery

| Exercise | Passing result |
| --- | --- |
| Hold writer A after it allocates 52, start B on another connection, and pull from a third. Repeat with A committing and rolling back. | A reader never advances beyond an unpublished change. No duplicate versions or orphan log entries. Use independent PostgreSQL connections and file-backed SQLite connections. A single-connection in-memory test cannot establish this property. |
| Advance through hidden changes, a full page from a single table, a repeated row, a new write above H and an expanded parent above H. Interrupt and resume. | Pagination terminates, the cursor is monotonic, every authorized final state converges, and no private payload appears. |
| Push a duplicate, partial conflict, conflict-only edit and rejected create. Queue a second device edit during pull. | Only real mutations publish. Existing push answers, optimistic rows, conflict recovery and eventual reconciliation still work. |
| Sign up, convert a guest with multiple communities, join, leave and rejoin from two devices. Revoke access mid-backfill. | Direct and bulk paths publish each replicated consequence. Grants backfill, removals prune, and account-only data stays private. |
| Bootstrap after log pruning. Purge between pages with cursors below, at and above the floor. Restart with pending changes and unsent bytes. | Retained live rows bootstrap and deleted records stay gone. Valid cursors continue and stale devices resync without losing pending work. |
| Switch accounts/servers, simulate reset/restore, and send a v1 request. Exercise Rust HTTP and real JSON values above 2^53. | Foreign cursors trigger resync, v1 receives HTTP 426, and the device can apply and resume without numeric truncation. |

Add targeted regressions to existing core sync, purge, table and Axum suites. Run the multi-device scenarios against SQLite and PostgreSQL with `TEST_POSTGRES_URL` configured. Exercise the HTTP transport in addition to `InProcess`. Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --all-targets`, and `cargo audit`. Run the frontend checks if a status consumer changes. [Repository test instructions](../../README.md)

Update `docs/design/offline-sync.md`, README, extension examples and fixtures in the implementation change. Classify the new framework data contexts in `framework.toml`. The release entry is in `FRAMEWORK_CHANGELOG.md`. Regenerate `framework.lock` with `cargo run -p framework_check -- lock` after changes to framework files. [Template release rules](template-and-apps.md)

The Rust phase is complete when these checks pass using only the Rust server. Later, an Elixir projector can write the authorized projection and the corresponding change entry under the same commit contract. Its event-store checkpoint and command conflict policies belong to that later phase. [Elixir integration contract](../suggestions/elixir-workflow-contract.md)
