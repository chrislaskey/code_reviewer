# core_elixir: the sample remote backend

The Elixir app `core`, with modules `Core.*` and `CoreWeb.*`: the
Elixir counterpart of the Rust `core/`.

**Status: first slice, 2026-11-27.** An Elixir backend that owns its
PostgreSQL database alone and answers Axum over the
[backend protocol](../docs/design/backend-protocol.md). It replaces the
removed `phoenix/` reference app. It has no pages and no assets. The
Svelte pages, Tauri and Axum stay as they are, see
[the backend seam](../docs/design/backend-seam.md).

Axum doesn't call it yet. That is step 1 of the seam. Until then it is
tested on its own, below.

## Run it

It needs PostgreSQL. Its event store lives in the `event_store` schema of
the same database.

```sh
cd core_elixir
mix setup                 # deps, database, event store, migrations
mix phx.server            # http://localhost:4100, service token dev-service-token
mix test                  # TEST_DATABASE_URL, else postgres://postgres@localhost/core_test
```

In production, set `DATABASE_URL`, `BACKEND_SERVICE_TOKEN`, `SECRET_KEY_BASE`
and `PORT`.

## How it is built

| Part | Where | What it does |
| --- | --- | --- |
| HTTP | `lib/core_web/` | Phoenix, JSON only, on Bandit, its default server. `POST /backend/<resource>/<action>`, behind the service token (`plugs/service_token.ex`). One controller per resource; `FallbackController` turns `{:error, …}` into the protocol's error codes. |
| Schema | `lib/core/migrations/` | The Rust core's PostgreSQL migrations, `core/migrations/postgres/*.sql`, run as Ecto migrations with their own versions (`migrations/rust.ex`), so the schema is the Rust server's, table for table. They are read at compile time and kept in the build, so a release needs no copy. Only what Rust doesn't have is the backend's own: Oban's tables and Commanded's projection checkpoints. `mix core.migrate` runs them all; in a release, `Core.Migrations.migrate/0`. |
| Data contexts | `lib/core/data/` | One module per table, named after it, as `core/src/data/` is. Only they issue SQL. |
| Publishing | `data/synced_row.ex`, `data/app_sync_changes.ex`, `data/app_sync_versions.ex` | Every write to a synced row takes the next `global_sync_version` from the locked feed row, and records the change, in the row's own transaction. |
| Files, with Commanded | `lib/core/app/files/` | One aggregate per file, holding its links to communities. Commands: create, update, add to a community, remove from one. The aggregate deletes the file when its last link goes. |
| The projector | `app/files/projector.ex` | Writes `app_files` and `app_files_communities` from the facts, with their versions, in one transaction, with its checkpoint. It records the pushed event and its answer in `app_events` in the same transaction. The event arrives as the command's dispatch metadata, so the domain events carry no sync details. It uses strong consistency, so a push answers from the rows it just wrote. |
| Other resources | `lib/core/accounts.ex`, `app/users.ex`, `app/communities.ex`, `app/memberships.ex` | Plain Ecto writes, with the same publishing rule. Only files go through Commanded, as the sample. |
| Sync | `lib/core/sync.ex`, `sync/` | Push, pull and backfill, as `core/src/sync.rs` answers them: one snapshot per page, pages by visible rows, parents first, cursors as decimal strings. |
| Jobs | `lib/core/jobs/purge.ex` | The daily purge on Oban: the change log, the floor, and file tombstones. |

## Decisions to review

- **One database.** The event store and the read model share one
  PostgreSQL database, in separate schemas. That's one backup and one
  connection string. Commanded allows separate databases if they are
  ever needed.
- **Validation before dispatch.** The shape checks, membership and the
  upload check run in `App.Files` and `App.FilesCommunities` before a
  command is dispatched. They read the current rows. The aggregate
  serializes the decisions about one file, and dedupes requests by id.
- **The answer is recorded by the projector**, with the rows. A retry
  that reaches the aggregate after its fact was appended changes
  nothing. It finds the recorded answer once the projector has run.
- **A refusal is not recorded**, as in the Rust server. A retried
  event that was refused is decided again, so the answer can change if
  something changed in between, such as the actor joining the community.
- **A projector that fails stops the backend.** Commanded stops a
  handler on an error, and the application with it. The event is in the
  store, so the projector meets it again at the next start, until the
  code is fixed. That is honest, never a silent skip, but one bad event
  takes the backend down. A time on a whole second did that, until
  `Time.from_wire` gave every time microsecond precision.
- **A membership check can be stale by a moment.** A file decision
  reads memberships when the command is dispatched. A leave committed
  just before can be missed. The next pull prunes the link, which is
  acceptable for files, see the protocol.

## Not built yet

- Axum calling it (step 1 of the seam). Then the Rust sync tests run
  through Axum against this backend, as the conformance suite.
- Sign-in from a trial, resending a code, passwords, signing in a device,
  deletions, preferences, posts, community writes and commerce. They
  answer `501 not_implemented`.
- The protocol's rule that a restored backup gets a new feed id.
- Recording the accepted terms at sign-in, as Rust's `finish_proven_in` does.
- Splitting a Rust migration that someday holds a function or a `DO`
  block: `Core.Migrations.Statements` splits at every `;`.
