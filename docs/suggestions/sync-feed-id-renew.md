# A command to renew the Rust server's feed id

**Built, 2026-11-27**: `sync::feed_id_renew` and `app_web sync feed renew`,
with the restore step in the README's Fly.io section. Found filling in
the Elixir sample, which has `mix core.feed_id_renew`.

## The gap

The protocol says a restored backup gets a new feed id, see "Publishing
a change" in `docs/design/backend-protocol.md`. The Rust server's feed id
is made once, by the migration that creates `app_sync_versions`. A
backup restored over its database keeps the old id. The restored head
can be below what devices already saw, and a device whose cursor is at
or below it resumes there. The versions given out again after the
restore can then name other changes, and that device misses them.

## Suggestion

- `app_web sync feed renew`, a command of `axum/src/command_line`, which
  sets a new `feed_id` under the feed lock: every device resyncs at its
  next pull.
- The deployment docs say to run it after a restore, before the server
  serves again.
