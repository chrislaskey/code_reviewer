# Backend protocol

**Status: draft, 2026-11-27.** These are the operations a remote backend
answers for Axum, see [the backend seam](backend-seam.md). The sample
backend, `core_elixir/`, implements the operations marked **sample**.
Axum calls them with `BACKEND=remote`.
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
| 409 | `exists`, `full` |
| 410 | `account_deleted` |
| 422 | `sign_in_expired`, `wrong_code`, `unknown_device_code` |
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
  `global_sync_version`, `deleted_at` and `deleted_by_id`. `version` is
  1 for a new row, and goes up by one on every server write that changes
  the row. The time of that write is in the change log, not on the row.
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
  backup gets a new one, so every device resyncs: in the sample,
  `mix core.feed_id_renew`; on the Rust server, `app_web sync feed renew`.
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

**`users/update`** (sample). A person renames themselves, as
`users::update` does.

```json
{"actor_id": "…", "name": "Alice"}
→ User
```

- An event like any change a person makes, recorded in `app_events`.
  The same name writes nothing.
- The name is trimmed, 1 to 80 characters. Rust checks it first.
- A deleted or unknown user is `not_found`.

**`users/avatar_update`** (sample). `{"actor_id", "avatar_url"} → User`:
sets the user's picture, an event in their name. Rust signed the upload,
checked the file in its bucket, and sends its public URL.
**`users/avatar_urls_list`** (sample). `{} → {"avatar_urls": ["…"]}`:
every user's picture, deleted users' too, for Rust's daily sweep of the
bucket.

**`accounts/users/get`** (sample). The account of a user, for the
current user's page: its email and whether it is a trial.

```json
{"user_id": "…"}
→ AccountUser
```

`AccountUser` is the Rust struct: `id`, `email`, `confirmed_at`,
`trial_expires_at`, `terms_version`, `terms_accepted_at`, `inserted_at`,
`updated_at`, `deleted_at`. An unknown id is `not_found`.

**`accounts/sign_in`** (sample). Starts a sign-in for an address.

```json
{"email": "…", "sign_up_enabled": true, "locale": "en"}
→ {"pending": "…", "mail": {"kind": "sign_in_code" | "no_account", "to": "…", "locale": "en", "code": "123456" | null} | null}
```

- An address with an account gets a six-digit code.
- An unknown address with `sign_up_enabled` gets an account, a user and
  a code. That's a sign-up. The user's first community and membership
  come at `sign_in_confirm`.
- `pending` identifies the sign-in for `sign_in_with_code`.
- An unknown address without sign-up gets the `no_account` mail.
- Rust sends the mail after the answer. The backend never sends mail
  itself.

**`accounts/sign_in_with_code`** (sample). Takes the code that was
mailed, as `accounts::sign_in_with_code` does.

```json
{"pending": "…", "code": "123456"}
→ {"pending": "…", "new_account": true}
```

- The code is good for ten minutes and five wrong attempts. A wrong
  code is `wrong_code`, with `fields.code`.
- The answer's `pending` is for the last step. `new_account` says
  whether finishing makes the account, so the page says "Create
  account" rather than "Log in".
- Nobody is signed in yet.

**`accounts/sign_in_confirm`** (sample). Finishes the sign-in, as
`accounts::sign_in_confirm` does.

```json
{"pending": "…", "terms_version": "2026-11-22"}
→ {"user": User, "email": "…", "token": "…"}
```

- A first sign-in confirms the account. It creates the user's personal
  community, `"Personal"`, and their membership of it. Both are
  published.
- Every sign-in records `terms_version`, the terms Rust shows, and the
  time, on the account: `terms_version` and `terms_accepted_at`.
- `token` is new, and is returned this once. An expired or used
  `pending` is `sign_in_expired`.

**`accounts/sign_in_confirm_for_device`** (sample). The same last step,
for the desktop or phone app that started the sign-in in a browser.

```json
{"pending": "…", "challenge": "…", "terms_version": "2026-11-22"}
→ {"code": "…"}
```

- It records the terms as `sign_in_confirm` does, and answers a code
  for the device to collect, not a token.
- The code is good for five minutes, once, with the challenge it was
  made for. Keep it hashed with the challenge.

**`accounts/sign_in_with_device_code`** (sample). The device collects its
token.

```json
{"code": "…", "challenge": "…"}
→ {"user": User, "email": "…", "token": "…"}
```

- Rust turns the device's verifier into its challenge, so the backend
  only matches the code and the challenge together. A wrong, used or
  expired code is `unknown_device_code`.

**`accounts/resend_code_to`** (sample). `{"pending"} → {"email"}`: the
address a pending sign-in's code went to, for Rust's limit on how often
one address is mailed. An unknown or expired `pending` is `wrong_code`.

**`accounts/resend_code`** (sample). `{"pending", "locale"} → {"mail": Mail}`:
a new code for the pending sign-in, which starts its ten minutes and
its five attempts again. The old code no longer matches. Rust sends the
mail.

**`accounts/sign_out`** (sample). `{"token": "…"} → {}`. Forgets the token.

**`trial_users/count`** (sample). `{} → {"count": 3}`: how many trial
users there are, for the cap.

**`trial_users/create`** (sample). A trial user and their token, as
`accounts::trial_users::create` makes them.

```json
{"max": 100}
→ {"user": User, "email": "guest_…@example.com", "token": "…"}
```

- The account has a made-up email and `trial_expires_at`, a day on.
  The user has a community of their own, `"Guest"`, and their
  membership of it. Both are published, and recorded as the trial's
  events.
- `full` (503) when `max` trial users are there already. Rust checks a
  cap of 0 itself.
- Nobody else sees a trial's communities or joins them, and a trial
  joins nobody else's: `memberships/create` answers `not_found`.
- A trial makes 500 writes, its community and membership included.
  The next write is `trial_writes_full` (403), and writes nothing.
  The writes counted are the trial's rows of `app_events`.
- The backend removes a trial outright, with everything of theirs, a
  day after it ends. Trials are web-only, so no device holds their rows.

**`accounts/sign_in_from_trial`** (sample). A trial user's browser
signs up, as `accounts::sign_in_from_trial` does.

```json
{"trial_id": "…", "email": "…", "locale": "en"}
→ {"pending": "…", "mail": Mail | null}
```

- An address with a confirmed account gets that account's code. The
  trial is left as it is.
- Any other address is the trial signing up. The code goes to that
  address, and the pending sign-in keeps it. The trial's made-up name
  gives way to one from the address. `sign_in_with_code` answers
  `new_account: true`, and `sign_in_confirm` gives the account the
  address, confirms it, and ends the trial. It shows the trial's
  communities to everyone, and forgets the trial's tokens. An
  unconfirmed sign-up with the same address is deleted. A confirmed
  one makes the last step `wrong_code`.
- The address of another trial gets a pending that matches nothing.
- `not_a_trial` (422) when the user is not a trial.

**`accounts/preferences/get`** (sample). `{"user_id"} → Preferences`:
`{"locale": "es" | null, "theme": "dark" | null}`. A row whose value no
longer applies, a language since dropped say, reads as null.

**`accounts/preferences/update`** (sample). Sets one key, as
`accounts::preferences::update` does.

```json
{"user_id": "…", "key": "locale", "value": "es"}
→ Preferences
```

- An event like any change a person makes, recorded in `app_events`.
  The row is published to the user's devices, and nobody else's.
- `value` is null, to follow the device, or one the key takes: a
  language of `app::locales::SUPPORTED`, or a theme of
  `helpers::themes::SUPPORTED`. Rust checks it first. Any other is
  `invalid`, with the reason as the message.
- A code's mail is in the language the account chose, else the one
  asked for.

**`accounts/password/get`** (sample). `{"user_id"} → {"is_set": true}`:
whether the user has a password.

**`accounts/password/update`** (sample). Keeps a password's hash as
the user's, their first or in place of the one they had.

```json
{"user_id": "…", "password_hash": "$argon2id$…", "locale": "en"}
→ {"mail": {"kind": "password_changed", "to": "…", "locale": "en", "code": null}}
```

- Rust checks the password's length and hashes it. The backend keeps
  the hash as it is, in `accounts_credentials`, and never sees the
  password.
- The mail tells the account's email, in the language it chose. Rust
  sends it.

**`accounts/password/get_hash`** (sample). The account a password signs
in, and its hash, for Rust to check.

```json
{"email": "…"}
→ {"user_id": "…" | null, "password_hash": "$argon2id$…" | null}
```

- Only a confirmed account, not a trial, has a `user_id` here. Its
  `password_hash` is null when it has no password.
- Rust checks the password against the hash, or against a hash nobody
  has when there is none, so every refusal takes the same work. Then it
  calls one of the two below.

**`accounts/sign_in_with_password`** (sample).
`{"user_id", "terms_version"} → {"user", "email", "token"}`: finishes a
sign-in whose password Rust checked. It records the terms, as the last
step of a code's sign-in does. **`accounts/sign_in_with_password_for_device`**
(sample). `{"user_id", "challenge", "terms_version"} → {"code"}`: the
same for a device, with a code it collects as after
`sign_in_confirm_for_device`. An account that is no longer confirmed
and full is `sign_in_expired`.

**`accounts/deletions/code/create`** (sample).
`{"user_id", "locale"} → {"mail": Mail}`: a code that confirms deleting
the account, ten minutes, in place of any earlier one. Rust mails it,
kind `delete_code`. A trial is `trial_has_no_code` (422).

**`accounts/deletions/create`** (sample). Deletes the account, as
`accounts::deletions::create` does.

```json
{"user_id": "…", "code": "123456" | null, "locale": "en"}
→ {"mail": {"kind": "account_deleted", …} | null}
```

- A full user needs the code. A wrong one is `wrong_code`, and counts:
  the fifth deletes the code.
- A trial needs none, and goes outright, as its expiry would remove it.
  The answer has no mail.
- Everything of the user's is soft-deleted as an event in their name,
  so every device that pulled a row sees it go: each community nobody
  else is in, with its memberships and the files' links in it, a file
  with no link left deleted; their membership of every other one; and
  their user, named `"Deleted user"`, with no picture.
- Their preferences go outright, and their staff row. The account gets
  `deleted_at`, so its tokens are answered `account_deleted`.
- The mail is in the language the account chose, read before the
  preferences go. Rust sends it, and deletes the pictures.

Specified, not in the sample: and the commerce and ops operations of
`backend-seam.md` §2.

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
- The sample serves `app_users`, `app_files`, `app_files_communities`
  and `accounts_preferences` here. Joining and leaving go through
  `memberships/*`.
- A user's own row takes an update of its `name`, merged as any
  update is. A create is `server_only`, a change to someone else's
  row `not_yours`.
- A preference follows `accounts::preferences::apply_in`: the last
  write wins, with no conflicts. A create of a key the user already
  has writes over that row, and the answer carries it, so the device
  drops its own.
- A trial's write past its limit ends the push with
  `trial_writes_full`. The events before it stay applied.

#### What the person must still be able to see

However well a backend resolves edits on its own, every answer to a
pushed event falls into one of three outcomes. The device must be able
to tell them apart:

| Outcome | Answer | On the device |
| --- | --- | --- |
| **Resolved.** Applied as sent, or merged by the backend's rules, a Commanded process included | `applied`, with the resulting row | The change is done. The row shows the result |
| **Needs the person.** The backend can't choose between the values | `conflict`, with `yours` and `server` per field | Kept as a conflict, with Reapply or Let go |
| **Refused, final.** The backend won't apply it, and retrying won't help | `rejected`, with a stable `reason`, and the server's row when the device should take it | Kept and marked refused, with the reason. Nothing retries it on its own. The person can choose Try again or Let go |

Better automatic resolution only moves events into the first outcome. It
never removes the other two.

- A backend that can resolve a case must still answer `conflict` when
  its rules don't settle it.
- It must answer `rejected` with a reason when a decision is final.
- It must never turn a refusal into a silent drop.
- A temporary failure, such as a timeout or the backend being
  unavailable, is a transport error, not `rejected`. The device then
  keeps the event pending and retries it.

The Rust device shows every rejection to the person, except a create
the server already has a row for. A refused create keeps its row and
holds back the events that wait with it. A refused update takes the
server's row and holds back nothing. See "Events" in
`docs/design/offline-sync.md`.

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

**`files/duplicates_list`** (sample). `{"actor_id", "sha256s": […]} →
[FileWithState]`: each as `files/get` answers it.

**`files_communities/create`** and **`files_communities/delete`**
(sample). `{"actor_id", "file_id", "community_id"} → FileCommunity`.
Puts the file in a community, or takes it out.

## Conformance

The sample is checked by its own tests, against these rules, and by
the conformance suite, `core/src/app/files/conformance_tests.rs`: what a
device sees of sync, files and memberships, the same tests on the Rust
backend and against a remote one. That suite is what makes a second
backend trustworthy. How to run it is step 2 of `backend-seam.md`.
