# core_elixir: the sample remote backend

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
mix run --no-halt         # http://localhost:4100, service token dev-service-token
mix test                  # TEST_DATABASE_URL, else postgres://postgres@localhost/core_elixir_test
```

In production, set `DATABASE_URL`, `BACKEND_SERVICE_TOKEN` and `PORT`.

## How it is built

| Part | Where | What it does |
| --- | --- | --- |
| HTTP | `lib/core_elixir/web/router.ex` | `POST /backend/<resource>/<action>`, behind the service token. Plug with Bandit, not Phoenix: a private JSON API with one router needs no more. |
| Data contexts | `lib/core_elixir/data/` | One module per table, named after it, as `core/src/data/` is. Only they issue SQL. The synced tables match the Rust PostgreSQL schema column for column. |
| Publishing | `data/synced_row.ex`, `data/app_sync_changes.ex`, `data/app_sync_versions.ex` | Every write to a synced row takes the next `global_sync_version` from the locked feed row, and records the change, in the row's own transaction. |
| Files, with Commanded | `lib/core_elixir/app/files/` | One aggregate per file, holding its links to communities. Commands: create, update, add to a community, remove from one. The aggregate deletes the file when its last link goes. |
| The projector | `app/files/projector.ex` | Writes `app_files` and `app_files_communities` from the facts, with their versions and the answer to the request, in one transaction, with its checkpoint. It uses strong consistency, so a push answers from the rows it just wrote. |
| Other resources | `lib/core_elixir/accounts.ex`, `app/users.ex`, `app/communities.ex`, `app/memberships.ex` | Plain Ecto writes, with the same publishing rule. Only files go through Commanded, as the sample. |
| Sync | `lib/core_elixir/sync.ex`, `sync/` | Push, pull and backfill, as `core/src/sync.rs` answers them: one snapshot per page, pages by visible rows, parents first, cursors as decimal strings. |
| Jobs | `lib/core_elixir/jobs/purge.ex` | The daily purge on Oban: the change log, the floor, and file tombstones. |

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
- **A refusal is not recorded.** A retried event that was refused is
  decided again. That answer could differ if things changed in between.
  The contract asks for durable refusals for strict workflows. Files
  don't need them.
- **A membership check can be stale by a moment.** A file decision
  reads memberships when the command is dispatched. A leave committed
  just before can be missed. The next pull prunes the link, which is
  acceptable for files, see the protocol.

## Not built yet

- Axum calling it (step 1 of the seam). Then the Rust sync tests run
  through Axum against this backend, as the conformance suite.
- Sign-in from a trial, resending a code, passwords, device codes,
  deletions, preferences, posts, community writes and commerce. They
  answer `501 not_implemented`.
- The protocol's rule that a restored backup gets a new feed id.
