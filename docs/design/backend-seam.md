# Backend seam: analysis

**Status: analysis, 2026-11-27.** Nothing is built yet. This follows
[the global sync version review](../suggestions/global-sync-version-review.md).

## Decisions this starts from

- Core gets a seam, as Svelte's `lib/core` has one. A single Rust entry
  point checks the shape of the input. Then it calls a backend.
- There are two backends. The **Rust backend** is today's server code,
  on SQLite or PostgreSQL. The **remote backend** is a service in
  another language. The sample is Elixir with Commanded.
- **One owner per server database.** The remote backend owns its
  database alone. Rust never reads or writes it.
- **The device is always Rust**, with its own SQLite database.
- **An app chooses its backend once**, when it is set up. It never
  switches.
- The template ships **the protocol plus a sample**. The sample's synced
  resource is **files**, with their communities.

## 1. Where the seam goes

The device never talks to the backend. It talks to Axum, over the sync
protocol and the HTTP API. Those stay the same whichever backend the
server uses. So the seam is server-only. The device code (`Role::Device`,
the sync engine, the local copies of bytes) is not touched.

Axum already calls only core's public functions. For files these are
`files::list`, `get`, `create`, `update`, `delete`, `content_url`,
`content_disposition`, `row_content`, `files::uploads::create` and
`files::duplicates::list`. Auth calls `users::get_by_token`, and sync
calls `sync::push`, `pull` and `backfill`. **Those public functions are
the seam's surface.** The `*_in` functions that borrow a transaction stay
inside the Rust backend.

Core already branches on `Role::Server` and `Role::Device` inside each
write, mostly in `apply_in`. The smallest correct seam leaves that alone,
and dispatches once at each public entry:

```text
files::create(deployment, user, new)
  1. new.checked()?                      shape only, always Rust
  2. match deployment.backend
       Rust   → today's body: the device or server branch, the data contexts
       Remote → remote.files_create(user, accepted)
```

The Rust backend is the code as it is now. The remote backend is a
client for the protocol. This keeps the device and the Rust server
sharing their validation and merge code, as they do today. It also
means step 0 changes no behavior.

### The rule that falls out: in remote mode, Axum has no database

If the backend owns the database, Axum in remote mode needs no database
connection at all. That is the test of the seam: start Axum with no
`DATABASE_URL`, and every route either works through the backend or is
refused. What this touches:

| Thing that uses the server database today | In remote mode |
| --- | --- |
| Token lookup on every signed-in request (`users::get_by_token`) | A backend operation. Cache it briefly in `ServerState` if the latency matters. |
| `app_jobs`, which records the runs of the job runner | The backend schedules its own maintenance (Oban). Rust runs no database jobs. |
| Rate limits, server settings, `ServerState` | These have no database already. They stay in Rust. |
| Mail | Rust sends it. The backend returns what to send (address, language, kind and code). Or the backend sends it. Decide once (§6). |
| The storage bucket | Rust keeps the keys. It signs uploads and downloads, and checks that bytes arrived. The backend owns the rows. |

## 2. Inventory by area

Every database-touching operation falls into one of three kinds:

- **Database**: behind the seam. The backend owns it.
- **Effect**: no database. Rust does it, with what the backend tells it.
- **Scheduled**: maintenance on a timer. The backend that owns the data
  runs it.

| Area | Public operations (core) | Kind | Remote backend must |
| --- | --- | --- | --- |
| Auth | `users::get_by_token`, sessions in `accounts_tokens` | Database | Resolve a token to the current user and their `kind`, on every request |
| Accounts | `accounts::sign_in`, `sign_in_confirm`, `sign_in_from_trial`, password, device codes, `deletions::*`, `trial_users::*` | Database + mail effect | Implement them. Return mail to send, or send it |
| Preferences | `accounts::preferences::*`, synced | Database | Implement it as a synced table |
| App resources | `communities`, `memberships`, `posts`, `posts_communities`, `users`, `avatars`, `current_community`, `local_users` | Database | Implement whatever the app uses. The sample does memberships and communities only, as needed by files |
| **Files (sample)** | `files::*`, `files_communities::*`, `files::uploads`, `files::duplicates` | Database + bucket effect | §3 |
| Sync | `sync::push`, `pull`, `backfill` | Database | Implement the whole protocol (§4) |
| Commerce | `checkouts`, `plans`, `products`, `prices`, `purchases`, `subscriptions`, Stripe and App Store events and reconciles | Database + provider effects | Out of the sample. Specified in the protocol. Refused until built |
| Ops | `ops::staff` | Database | Out of the sample |
| Jobs | `Purge`, `TrialUsersDelete` | Scheduled | Run its own (Oban) |
| Jobs | `FilesDeleteUnreferenced`, `AvatarsDeleteUnreferenced` | Scheduled + bucket effect | Provide "list stored keys of live rows". Rust keeps the sweep, since Rust holds the bucket |
| Jobs | `StripeReconcile`, `AppStoreReconcile` | Scheduled + provider effect | Out of the sample |
| Device only | `create_with_bytes`, `local_copy`, `cache_get`, `cache_delete`, `sync::run`, `local_users`, `device_settings` | Not on the seam | Nothing |

The protocol has to name every row of this table. The sample implements
auth, the sign-in accounts flow, sync, memberships and communities as
files needs them, and files. Everything else answers "not implemented"
in the sample, and the conformance suite marks it so.

## 3. Files as the sample resource

Files are a good test. They exercise more of the seam than posts would:

- a bucket effect, where Rust must verify bytes the backend then
  believes;
- a child table (`app_files_communities`) with its own sync events;
- a rule across aggregates: you must be a member of each community;
- a lifecycle rule: removing a file from its last community deletes it.

### Who does what

| Step | Rust (Axum + core) | Backend |
| --- | --- | --- |
| Ask to upload (`files::uploads::create`) | Checks the size and type. Refuses a trial, from the current user's `kind`. Signs the PUT URL. Answers "already there" when the bucket has the bytes | Nothing |
| Create (`files::create`, or a pushed create) | Checks `FileNew` and the community ids. Checks the bucket has the bytes (`bytes::is_uploaded`) | Checks membership of each community. Rejects a duplicate id. Creates the file and its links. Answers with the row |
| Rename (`files::update`, or a pushed update) | Checks the name | Merges field by field against `base_version`. Answers applied or conflict, with the row |
| Change communities (`files_communities::create`, `delete`, `set_in`) | Checks the ids | Checks membership. Revives a link instead of making a new one. Deletes the file when its last link goes |
| List, get, search, duplicates | Checks the filter limits | Answers the visible rows, with each file's state, creator and communities |
| Download (`content_url`, `content_disposition`, `row_content`) | Signs the GET URL, or serves the content kept on the row | Answers "may this user see file X", with its name and storage key |
| Bucket sweep | Lists the bucket, deletes keys no live row has | Lists the storage keys of live rows |

The trust point: the backend takes Rust's word that the bytes were
uploaded. That's safe because only Axum can reach the backend, over an
authenticated private connection. The protocol must say so, so the
backend never takes `uploaded` from the device.

### Commanded shape for the sample

- **One aggregate per file**, stream `file-<id>`. Its links to
  communities belong to it. Then "delete when its last community goes"
  is a decision inside one aggregate, and it serializes with renames.
- **Commands:** `CreateFile`, `RenameFile`, `AddFileToCommunity`,
  `RemoveFileFromCommunity`. Each carries the sync event's id, which
  identifies the request.
- **Events:** `FileCreated`, `FileRenamed`, `FileAddedToCommunity`,
  `FileRemovedFromCommunity`, `FileDeleted`.
- **Mapping a push.** An `app_files` event becomes create or rename. An
  `app_files_communities` event becomes add or remove on the file's
  aggregate. For a remove, the file id comes from the link row the
  backend already has.
- **The answer needs the row.** Push answers each event with the
  resulting row, and the device installs it. So the command handler must
  answer from the projection written in the same dispatch (strong
  consistency on the sync projector), or build the row from the
  aggregate's state. The sample should prove which one works.
- **Membership is another aggregate.** "Is this user in community C" is
  read from the membership projection when the command is decided. A
  revocation committed a moment earlier may not be visible yet. For
  files that window is acceptable: the next pull prunes the link. The
  protocol should say the window is allowed for this rule.
- **The sync projector** writes the `app_files` and
  `app_files_communities` rows. It stamps each with the next
  `global_sync_version` in the same transaction. One ordered projector
  for every synced table is the simplest choice. Its own counter then
  supplies the version, and the lock contention of the Rust design
  doesn't arise.

## 4. Sync must be whole operations, not per-table calls

It is tempting to keep the pull algorithm in Rust: the paging, parent
ordering and continuations. Rust would then ask the backend per table,
"rows of app_files visible to U in (after, H]". But a page must be read
from **one consistent snapshot**: the head, the floor, the visibility
checks and the rows. A string of remote calls can't give that. So
`push`, `pull` and `backfill` are each one backend operation. Rust's part
is the protocol header, auth, size limits and passing the request on.

That makes the sync protocol the largest part of the remote contract.
The visibility rules for communities, parent-first order, backfill on
join, the version floor and resync all move into the backend. The way to
keep two implementations honest is to reuse the tests we already have.
The sync tests in `core/src/sync.rs` already drive a server through a
`Transport` (`InProcess` today, `Http` behind `sync-http`). Run the same
tests against an Elixir server over `Http`, and they become the
conformance suite.

## 5. Step 0: refactors before the seam

These are worth doing first, on the Rust code alone, with no behavior
change. Low-level contexts (`data/`) mostly conform to the pattern
already. SQL lives in `data/`, and high-level modules call contexts, not
adapters. The work is mostly in the high-level layer.

1. **Give each public high-level function one shape:** check the input,
   then do the work. Today some checks happen inside the transaction,
   mixed with reads. Pulling the shape checks to the top makes them
   reusable in front of the remote backend. `files::update` is already
   close; `files::create` is fine. Audit the rest of the surface in §2.
2. **Mark the seam's surface.** List exactly the public functions Axum
   and the jobs call. Keep everything else `pub(crate)`. Today some
   internals are `pub` and some entries are `pub(crate)`, such as
   `files::list_all`. The list should match §2.
3. **Split `SyncedTable` into its server half and its device half.**
   - Server half: `apply_in` on the server, `list_synced_after`,
     `list_live_in_community_after`, `parents`.
   - Device half: `apply_server_row`, `delete_row`, `prune`,
     `delete_for_resync`.

   A Rust-backend app implements both. A remote-backend app implements
   only the device half in Rust. The server half is then the backend's
   job.
4. **Bring the new sync modules into the low-level pattern.**
   `data/app_sync_changes.rs`, `data/app_sync_versions.rs` and
   `row_version`/`clear_versions` in `data/app_sync_state/mod.rs` write
   SQL inline, with a local `query!` macro. Every other context has
   private `sqlite.rs` and `postgres.rs` adapters. Give these the same.
5. **Name contexts by the convention.** `app_files::search` and
   `app_files::list_with_sha256s` are lists with a filter. Make them
   `list` with a filter type. `files::forget_in` and
   `files::delete_with_last_community` describe what they do, so they can
   stay. Check the other `data/` modules for the same.
6. **One context that opens its own transaction:**
   `data::device_settings::upgrade` calls `deployment.begin()`. Contexts
   should borrow the caller's transaction. Move the transaction to the
   caller.
7. **Fold in the review's fixes** that change the Rust backend anyway:
   - narrow the feed lock;
   - page pulls by visible rows;
   - make the first sync resumable;
   - stamp rows inside the data layer.

   See the review, §3.

None of these needs the remote backend to exist. Each can be verified by
the current test suite.

### As built (2026-11-27)

Done:

- **4.** `app_sync_changes` and `app_sync_versions` are directories with
  private `sqlite.rs` and `postgres.rs` adapters. `app_sync_state` keeps
  all its SQL in its adapters.
- **5.** `app_files::list` takes a `Filter`, as `app_posts::list` does. It
  replaces `list_with_sha256s`. `search` stays: it answers a count with a
  page, which a list does not.
- **7, stamping.** The `create` and `update` of each synced table's data
  context take the `Role`. They stamp the row with
  `app_sync_changes::stamp` and return it. No write path stamps by hand.
  The bulk trial conversion still records each row itself.
  `assert_every_row_published` runs after every sync in the core sync
  tests, and fails on a server row whose stamp is not its latest change.
- **7, paging.** A pull pages by the rows the user sees, as bootstrap did.
  The change log is now used only for the floor.
- **7, resuming.** `app_sync_state.continuation` keeps an unfinished first
  pull's next page. `app_sync_backfills.continuation` does the same for
  each backfill. Two tests cut the network after a page and check that
  the next sync goes on from it.
- **7, the feed lock.** A sign-in code request takes the lock only to
  sign someone up. A retried event is answered before the lock. The
  lock still comes before an `apply_in` reads the row it decides on.
  Going further needs row locks (`SELECT … FOR UPDATE`) in each context,
  and a tested lock order.

Moved to step 1, the seam itself:

- **1 and 2.** These become concrete function by function, as each entry
  gets its dispatch. Changing visibility before then would be guesswork.
- **3.** Splitting `SyncedTable` adds a second list to every app's
  `synced_tables.rs`. Nothing yet needs only one half. The remote
  backend is the first that will, so the split waits for it.
- **6.** Moving `device_settings::upgrade`'s transaction to its caller
  would put a transaction into Tauri. The better fix is to move the
  function to a high-level module, which is a change of its own.

Still open from the review: the membership backfill trigger, and the
purge holding the feed lock (§3.5).

## 6. Decided (2026-11-27)

- **Mail.** The backend returns the mail to send: its kind, address,
  language and code. Rust sends it after the backend commits. Rust keeps
  the mailer and the translations.
- **Axum to the backend.** Plain HTTP with JSON and a shared service
  token, on a private network. The device-facing JSON types in core are
  reused as the wire types.
- **Token lookup.** Every signed-in request asks the backend, with no
  cache. A cache comes later, if the latency matters. See
  `docs/suggestions/backend-token-cache.md`.
- **Routes.** The remote backend implements only the operations. Axum
  keeps the routes, cookies, CSRF checks and the page server.
- **The protocol** lives in `docs/design/backend-protocol.md`, with each
  operation's request, answer and errors. The conformance tests name it.
