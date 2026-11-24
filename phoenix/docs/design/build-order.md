# Build order

The order to implement `multi-tenancy.md` and `offline-sync.md`. Foundations
first, each phase ending in a state that works, is tested, and is worth
keeping even if the next phase never happens. Anything that is not needed to
prove the phase sound is pushed later.

Two rules for reading this:

- A phase is done when its "Proves" list has tests and its "Delivers" state
  is usable from the web app or `curl`.
- Generators are extracted after the first hand-written version of a
  pattern exists, never before. See "Later."

| Phase | Name | What exists afterward |
| --- | --- | --- |
| 0 | Groundwork | UUIDv7 ids, soft-delete columns, pinned SQLite formats |
| 1 | App users | `app_` tables, and the app's own view of each user |
| 2 | Communities | Membership rows, join, leave, rejoin |
| 3 | First resource | A tiny post, in the communities its owner chose, hand-written |
| 4 | Sync server | Push, pull, and backfill over HTTP, driven by `curl` |
| 5 | Client schema export | Generated SQLite migrations for clients, kept fresh by a test |
| 6 | First offline client | Desktop app syncs offline-first |
| 7 | Lifecycle | Account deletion and the 90-day purge |
| Later | Nice to have | Comments, nudges, attachments, generators, SSO, Postgres |

## Phase 0: Groundwork

Changes to the existing app that everything later assumes. No new concepts.

Builds

- `users` and `user_tokens` move to UUIDv7 primary keys,
  `autogenerate: [version: 7]`.
- `deleted_at` and `deleted_by_id` on `users`, with a partial unique index
  on email where `deleted_at is null`.
- Every datetime column that will ever sync is `:utc_datetime_usec`. Pin
  the SQLite adapter defaults in config so they cannot drift: binary ids as
  text, datetimes as ISO 8601.
- Enumerated columns are strings with `Ecto.Enum` in the schema. No
  database enum types, no check constraints, so a new value never needs a
  migration.
- `App.Data.Context` applies `deleted_at is null` for schemas that have the
  column.

Delivers: the same app as today on new ids.

Proves: existing auth tests pass; a soft-deleted user cannot log in; the
same email can register again.

Deferred: everything else.

## Phase 1: App users

The namespace every later table lives in. One deployment is one tenant
that needs isolation, with one database; see `multi-tenancy.md`.

Builds

- Every table the app owns is prefixed `app_`. `users` and `users_tokens`
  stay as they are: the accounts, which another product in the same
  deployment would share.
- `app_users` (id, email, `joined_at`), the same id as `users`, written in
  the same transaction as the user at registration. Its email follows an
  email change. Every foreign key of a later `app_` table points here.

Delivers: every registered user has an app user row.

Proves: registration writes both rows or neither; an email change updates
both.

Deferred: profile fields beyond the email.

## Phase 2: Communities

Membership, with the leave and rejoin semantics that phase 3
depends on. No permissions.

Builds

- `app_communities` and `app_users_communities` (user_id, community_id,
  role). `role` is a string column with an `Ecto.Enum` whose only value is
  `member` for now.
- Create community, open to any user.
- Join, leave, rejoin. Leave soft-deletes the membership with one
  `deleted_at` value computed once, so later cascades share it. Rejoin
  within the retention window clears `deleted_at` on the existing row.
- `App.Web.Scope` gains `community`, resolved from the path by `on_mount`
  or a plug, and checked against `users_communities`.
- Minimal web UI: list communities, create, join, leave.

Delivers: users create and join communities.

Proves: non-members cannot read a community; leave then rejoin restores the
same membership row; a soft-deleted membership is invisible everywhere.

Deferred: every permission check, the `admin` value, discoverability and
public joinability, invitations, nested communities (never).

## Phase 3: First resource

The smallest resource that exercises the whole model: a post, written by one
owner, visible in the communities the owner chose. Hand-written, as the
reference for every rule the generator will later copy.

Builds

- `app_posts` (id, owner_id, body), with the sync columns and soft
  delete, and its stream in `app_events`, the server-only log of every
  change to a post, see `related-resources.md`.
- `app_posts_communities` (post_id, community_id, users_communities_id), with
  a plain unique index on (post_id, community_id): adding a post back
  revives the row rather than inserting a new one, like rejoining does
  for `app_users_communities`.
- Add to and remove from a community, owner only. Adding back clears
  `deleted_at` on the same row.
- The visibility query: inside a community scope, a join on
  `app_posts_communities`; across communities, "rows for communities where the
  user has a live membership, or owned by the user."
- A post in zero communities is visible only to its owner.
- The leave cascade extended to the member's `app_posts_communities` rows,
  using the membership's `deleted_at`. Rejoin revives exactly those rows.
- Minimal web UI: write a post, tick the communities it is in, read a
  community's feed.

Delivers: a post written once appears in the communities its owner chose
and nowhere else.

Proves: a post in two of three communities is invisible in the third; a
post by another user in no shared community is invisible; removing a post
from one community leaves it in the others; leaving removes the owner's
posts from that community and rejoining within the window brings them
back; a row the owner removed on purpose does not come back on rejoin; a
soft-deleted post is gone from every feed, and its community rows are
deleted with it, see "Deletes" in `related-resources.md`.

Deferred: a title or any other field, comments, reactions, moderation of
any kind, the generator.

## Phase 4: Sync server

Everything the server needs for offline clients, driven and tested with
`curl` before any client exists.

Builds

- The sync columns on every synced table: `version` (server-set, the MVCC
  version), `updated_at` (usec), `last_synced_at` (server-set in the write
  transaction, not null), `deleted_at`, `deleted_by_id`. A synced-table
  marker on each schema.
- Auth exactly as the websocket `/api` does it: the same API token from
  `user_tokens`, sent as `Authorization: Bearer` on HTTP instead of in the
  websocket upgrade header. One token, two transports.
- `POST /sync/pull`: rows visible to the user with `last_synced_at` later
  than the cursor minus the overlap buffer, ordered by `(last_synced_at,
  id)`, paged, every returned `app_posts_communities` row carrying its post.
- `POST /sync/push`: one transaction per batch of rows and events,
  dependency order, everything authorized against the scope, datetimes
  rejected unless UTC with six fractional digits and bounded by the server
  clock. Membership and `app_posts_communities` rows are server-wins.
  Posts and profiles change only through events, merged field by field,
  with a field someone else changed first returned as a conflict. Response lists accepted rows with their new
  `last_synced_at` and `version`, rejected rows with the server's version,
  and an answer per event.
- `app_user_profiles` (user_id, display_name, bio): a user's public
  details, one per user, created at registration, edited only by them on
  the web and on devices. The reference for a resource with no server
  event log.
- `POST /sync/backfill/:community_id`: every live `app_posts_communities` row
  for the community, each carrying its post, paged. Called by a client when
  it sees a new membership for its own user. See "What backfill means."
- A sync test harness: two simulated clients as plain Elixir maps, pushing
  and pulling in interleaved orders.

Delivers: a shell script can create a post on "device A," pull it on
"device B," edit on both, and end with both matching the server and the
later device holding its edit as a conflict.

Proves: the same field edited on two devices keeps the edit that reached
the server first and returns the other as a conflict; different fields
both apply; a retried event applies once; a future-dated deletion is
bounded to server time; a row pushed for a
community the user left is rejected; an `app_posts_communities` row for a newly
joined community arrives with its post; the backfill returns a community's
existing posts; the overlap buffer re-delivers without duplicating.

What backfill means: a device's pull asks for rows changed since its cursor.
When the user joins a community, thousands of posts added there long ago
become visible without any of them changing, so the cursor never finds
them. Backfill is the device asking the server for everything currently in
that one community, regardless of cursor, once, right after it learns of
the new membership.

Deferred: the purge job and purge horizon (phase 7), the websocket nudge,
attachments, `server_seq`.

## Phase 5: Client schema export

The bridge from server to clients.

Builds

- `mix app.migrations`: runs each migration touching a synced table against
  a scratch SQLite database with SQL logging on, and writes
  `priv/client_migrations/<version>_<name>.sql`.
- A test that fails when any exported file is missing or stale, in the
  style of the Svelte mirror check.
- A short README in `priv/client_migrations` on how clients apply them and
  which tables they must add themselves (`sync_state`, `sync_outbox`).

Delivers: committed SQL files a client can apply in order.

Proves: the exported schema, applied to an empty SQLite file, accepts a row
pulled from the server unchanged.

Deferred: nothing; this phase is small.

## Phase 6: First offline client

The Tauri desktop app, since its Rust channel client already exists. Mobile
repeats the pattern later.

Builds

- A profile on the device: the user's id and email from the `auth.user.get`
  request, kept for offline use.
- One SQLite file, created from the exported migrations, plus `sync_state`,
  `sync_outbox` and `sync_events`.
- Local writes: upsert the row with a client-generated UUIDv7 and
  `updated_at` from the device clock, and upsert the outbox entry, or for
  a post queue an event.
- Conflicts: "N changes were not saved on the server," each to reapply
  or let go.
- A sync worker: push the outbox, pull with the cursor, apply
  rows, run the reachability prune, request a backfill when a new
  membership for this user arrives. "Full resync required" is added in
  phase 7 with the purge.
- UI reads only from the local file.

Delivers: the desktop app works with the network off and converges when it
comes back.

Proves: edits made offline appear on the web after reconnecting; a post
removed from a community on the web disappears from the device; joining a
community on the web fills the device with that community's posts;
rejoining restores them; the same post edited on both sides while offline
keeps the edit that reached the server first, and the other reapplied
reaches both.

Deferred: mobile apps, one device on several deployments, background sync,
realtime.

## Phase 7: Lifecycle

Operations that must exist before real users, not before the model is
proven.

Builds

- The purge job: hard-delete rows soft-deleted more than 90 days ago, in
  dependency order, recording the purge horizon. The pull response carries
  the horizon and a "full resync required" signal when a client's cursor is
  older; the client drops its file and pulls fresh.
- Account deletion, in one transaction: soft-delete the `users` row and its
  `app_users` row, delete the tokens, leave every community, which takes
  the user's posts there along, and delete the user's posts. The purge
  removes it all at 90 days.

Delivers: a user can delete their account, and soft-deleted data does not
accumulate forever.

Proves: after deletion the email can register again; the deleted user's
posts vanish from every feed; a device of another member receives
the deletions through sync; a device away longer than 90 days resyncs from
nothing and ends up matching.

## Later

Worth doing, none of it needed to prove the design.

- **Comments and reactions** hanging off the `posts_communities` row, with
  the leave cascade and the carrying rule in pull.
- **Tenant operations**: exporting a deployment by copying its file, and
  backups. The design makes these file operations; the tooling around them
  can wait.
- **The "something changed" nudge** over the existing channel.
- **Attachments**: upload-then-reference, a client upload queue.
- **The community-scoped resource generator**, extracted from phase 3's posts.
- **Mobile clients**, repeating phase 6.
- **Public, discoverable, joinable communities.**
- **Multiple credentials per account and SSO** over OIDC, which also gives
  one login across deployments.
- **Permissions in the database** replacing the role strings.
- **Postgres**, when a deployment's write rate demands it. The overlap buffer becomes mandatory then.
- **`server_seq`, event sourcing proper.** Deriving rows from their event
  logs. The design leaves room; nothing requires them.
