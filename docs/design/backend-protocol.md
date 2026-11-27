# Backend protocol

**Status: draft, 2026-11-27.** These are the operations a remote backend
answers for Axum, see [the backend seam](backend-seam.md). The sample
backend, `core_elixir/`, implements the operations marked **sample**.
The rest are named here, so the protocol is complete. The sample answers
them `501 not_implemented`.

## Transport

- Axum calls the backend over HTTP, on a private network. Every request
  is a `POST` with a JSON body, to `/backend/<resource>/<action>`.
- Every request carries `authorization: Bearer <service token>`. The
  token is a secret Axum and the backend share (`BACKEND_SERVICE_TOKEN`).
  The backend refuses any other request with `401`.
- An operation done for a person carries `"actor_id"`: the id of the
  user Axum resolved from their token, by `users/get_by_token`. The
  backend trusts it, since only Axum can reach the backend. It
  authorizes every read and write for that user, as the Rust server does.
- An answer is `200` with the operation's JSON, or an error:

  ```json
  {"error": {"code": "not_a_member", "message": "…", "fields": {"name": ["…"]}}}
  ```

  `code` is stable, and Axum maps it to the person's words and to an
  HTTP status. `fields` is present for a validation error, as
  `FieldErrors` in core.

| Status | Codes |
| --- | --- |
| 400 | `invalid` (with `fields`), `invalid_continuation` |
| 401 | `unauthorized` (a wrong service token), `unknown_token` |
| 403 | `not_a_member` |
| 404 | `not_found` |
| 409 | `exists` |
| 410 | `account_deleted` |
| 422 | `sign_in_expired`, `wrong_code` |
| 501 | `not_implemented` |

## Shared shapes

The rows are the Rust structs as `serde` writes them. The device stores
them as they arrive, so the backend must write exactly these fields.

- **Times** are RFC 3339 in UTC, with microseconds
  (`2026-11-27T10:00:00.123456Z`). **Ids** are UUIDs in hyphenated
  lower case. The backend makes new ids as UUIDv7.
- **`global_sync_version`** is a decimal string on a row, or `null` on a
  row a device made that the server has not yet accepted. So is the
  cursor's version. The backend never sends it as a JSON number.
- **Every synced row** has `id`, `inserted_at`, `updated_at`, `version`,
  `last_synced_at`, `global_sync_version`, `deleted_at` and
  `deleted_by_id`. `version` is 1 for a new row, and goes up by one on
  every server write that changes the row. `last_synced_at` is the
  server's time of that write.
- A row on the wire is a `TableRow`: `{"table": "app_files", "row": {…}}`.

| Table | Fields beside the sync columns |
| --- | --- |
| `app_users` | `name`, `avatar_url` |
| `app_communities` | `name` |
| `app_users_communities` | `user_id`, `community_id`, `role` (`"member"`) |
| `app_files` | `created_by_id`, `name`, `content_type`, `size`, `sha256`, `content` |
| `app_files_communities` | `file_id`, `community_id`, `created_by_id` |

### Publishing a change

The backend owns the feed. It must keep the same promise the Rust server
keeps, see [offline sync](offline-sync.md):

- Every write that changes a synced row gives the row the next
  `global_sync_version`, and records `(version, table, row id, time)` in
  its change log, **in the same transaction as the row**.
- Versions commit in the order they were given out. A reader must
  never see version N+1 committed while N is not. One locked counter
  row, as the Rust server has, does that. So does one ordered projector.
- The feed has an id, a UUID made when its database is made. A restored
  backup gets a new one.
- The floor is the lowest cursor a pull still answers. Purging the
  change log moves it.

## Operations

### Auth and accounts

**`users/get_by_token`** (sample). The user a token stands for, on
every signed-in request.

```json
{"token": "…"}
→ {"user": User, "email": "…", "kind": "account" | "trial"}
```

Errors: `unknown_token`, `account_deleted`. The backend keeps only
SHA-256 hashes of tokens, as hex. It matches them exactly.

**`accounts/sign_in`** (sample). Starts a sign-in for an address.

```json
{"email": "…", "sign_up_enabled": true, "locale": "en"}
→ {"pending": "…", "mail": {"kind": "sign_in_code" | "no_account", "to": "…", "locale": "en", "code": "123456" | null} | null}
```

- An address with an account gets a six-digit code.
- An unknown address with `sign_up_enabled` gets an account, a user and
  a code. That's a sign-up. The user's first community and membership
  come at `sign_in_confirm`.
- An unknown address without sign-up gets the `no_account` mail.
- `pending` identifies the sign-in for `sign_in_confirm`. Rust sends the
  mail after the answer. The backend never sends mail itself.

**`accounts/sign_in_confirm`** (sample). Finishes a sign-in with the code.

```json
{"pending": "…", "code": "123456"}
→ {"user": User, "email": "…", "token": "…"}
```

- The code is good for ten minutes and five wrong attempts. Errors:
  `sign_in_expired`, and `wrong_code` with `fields.code`.
- A first sign-in confirms the account. It creates the user's personal
  community, `"Personal"`, and their membership of it. Both are
  published.
- `token` is new, and is returned this once.

**`accounts/sign_out`** (sample). `{"token": "…"} → {}`. Forgets the token.

Specified, not in the sample: `accounts/sign_in_from_trial`,
`accounts/resend_code`, the password operations, device codes,
`accounts/deletions/*`, `trial_users/*`, `accounts/preferences/*`, and
the commerce and ops operations of `backend-seam.md` §2.

### Sync

The bodies are core's `PushRequest`, `PullRequest`, `BackfillRequest`,
`PushResponse` and `Page`, with `actor_id` added. Their rules are in
[offline sync](offline-sync.md). Here is what the backend must do.

**`sync/push`** (sample).

```json
{"actor_id": "…", "events": [Event], "uploaded": ["<event id>", …]}
→ {"events": [Answer]}
```

- It applies the events in order, each in its own transaction, and
  answers every one.
- `uploaded` lists the file creates whose bytes Rust found in the
  bucket. A file create not in it, with no `content`, is rejected
  `file_not_uploaded` and writes nothing.
- A retried event, by its `id`, gets the answer it got the first time,
  with the row as it is now.
- An update merges field by field against `base_version` and `base`,
  as `sync::events::merge` does. A field someone else changed is a
  conflict, `{"yours": …, "server": …}`. The fields that merge apply.
- An event of a table the backend does not serve is rejected
  `unknown_table`.
- The sample serves `app_files` and `app_files_communities` here.
  Joining and leaving go through `memberships/*`.

**`sync/pull`** (sample).

```json
{"actor_id": "…", "cursor": Cursor | null, "after": Continuation | null, "limit": 200}
→ Page
```

- One page is read in one snapshot: a `REPEATABLE READ READ ONLY`
  transaction.
- The first page's bound is the feed's head. A continuation keeps the
  bound it carries.
- A page holds the rows the actor sees with a version in
  `(after, bound]`. It takes the first ones from each table, merges them
  by version, and cuts at `limit`. `next` continues after the last row.
- Each row goes after its parents: a membership after its user and
  community, a file community row after its file and community. A
  parent is sent once a page.
- `cursor` is set on every page of an incremental pull. On a bootstrap
  (no cursor) it is set on the last page only. An intermediate page's
  cursor is its last row's version. The last page's cursor is the bound.
- `resync: true`, with no rows, answers a cursor or continuation from
  another feed, one below the floor, or one above the head.

The rows a user sees:

| Table | Visible to user U |
| --- | --- |
| `app_users` | U, and every live member of a community U is a live member of |
| `app_communities` | Every community, except those of another user's trial; a trial user sees only their own |
| `app_users_communities` | U's own, and every row of a community U is a live member of |
| `app_files` | Files with a live row in a community U is a live member of |
| `app_files_communities` | Every row of a community U is a live member of |

Deleted rows go out like any other, as tombstones.

**`sync/backfill`** (sample). The same `Page`, of the live rows of
`app_files_communities` in one community, with their parents. The
answer is `403 not_a_member` when the actor has no live membership of
it. Its continuation is its own. It never carries a `cursor`.

### Communities and memberships

**`communities/list`** (sample). `{"actor_id"} → [CommunityAndMembership]`,
as `{"community": Community, "membership": Membership | null}`:
each community the actor sees, by name, with the actor's live
membership or `null`.

**`memberships/create`** (sample). `{"actor_id", "community_id"} →
Membership`. Joins, or revives a membership left before. It is
published.

**`memberships/delete`** (sample). `{"actor_id", "community_id"} → {}`.
Leaves: it soft-deletes the membership, and publishes it. The actor's
file community rows stay.

Specified, not in the sample: `communities/create`, `update`, `delete`,
and the community rules for posts.

### Files

**`files/list`** (sample). `{"actor_id", "filter": FileFilter} → FileList`.
The live files the actor sees, newest first. The name match ignores
case. A content type matches by the type before any `;`. `total`
counts them all. Each file has its creator (`null` unless they are a
live member of one of the file's communities that the actor is in),
its communities among the actor's, `shown_as` and `"state": "synced"`.
`shown_as` comes from the type before any `;`: `text` for `text/plain`,
`pdf` for `application/pdf`, `image` for `image/*`, else `not_shown`.

**`files/get`** (sample). `{"actor_id", "id"} → FileWithState`, or
`not_found`.

**`files/create`** (sample). `{"actor_id", "file": FileNew, "uploaded": true} → File`.
The same rules as a pushed create: one community at least, each one
the actor is a live member of, and the bytes uploaded unless `content`
is set.

**`files/update`** (sample). `{"actor_id", "id", "name", "community_ids"} → File`.
Renames the file, and makes its communities among the actor's exactly
these.

**`files/delete`** (sample). `{"actor_id", "id"} → {}`. Takes the file
out of every community of the actor's. The file is deleted when those
were its last.

**`files/readable`** (sample). `{"actor_id", "id"} →
{"name", "content_type", "storage_key", "content"}`. What Rust needs to
sign a download, or to serve the content kept on the row. The storage
key is `files/<created_by_id>/<id>`.

**`files/storage_keys_list`** (sample). `{} → {"keys": […]}`. The keys
of every file row, deleted or not, for Rust's bucket sweep.

**`files/duplicates_list`** (sample). `{"actor_id", "sha256s": […]} → [File]`.

**`files_communities/create`** and **`files_communities/delete`**
(sample). `{"actor_id", "file_id", "community_id"} → FileCommunity`.
Puts the file in a community, or takes it out.

## Conformance

The sample is checked by its own tests, against these rules. Next comes
a Rust suite that runs core's sync tests through Axum against a remote
backend. That suite is what makes a second backend trustworthy, see
`backend-seam.md` §4.
