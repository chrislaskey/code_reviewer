# Files in communities

A file belongs to its communities, not to the person who created it. A
post is the other pattern: its user's, shown in the communities they
chose, gone when they leave or their account goes. A file stays as long
as one community has it, whoever created it and wherever they went. A
private file is a file in the person's `Personal` community, where they
are the only member.

`offline-files-decisions.md` holds how a file's bytes are kept, sent and
fetched; this holds who a file belongs to and who sees it. Phase 1 is
built, in `core/src/app/files_communities.rs` and `core/src/app/files.rs`;
phases 2 and 3 are to build, see "Build order".

## Decisions

- **A file lives as long as it is in a community.** Every file is in at
  least one, from its create. Taking it out of its last community
  deletes it, at the same moment, on the server.
- **Only members see a file**: a user sees a file in a community they
  are a live member of, and nothing else. Its creator has no access of
  their own; one who left every community of a file sees it no longer.
- **Leaving a community leaves its files there.** Nothing follows a
  membership, so a rejoin revives nothing.
- **Delete is taking a file out of the communities you are in.** It is
  deleted when that was the last one; in a community you are not in, it
  stays.
- **Any live member decides**, as for a community itself: a member of a
  community the file is in renames it and puts it in another community
  they are in; a member of a community takes it out of that community.
- **The bytes are at `files/<created_by_id>/<file id>`**, worked out
  from the row, whose `created_by_id` never changes. Before a file has
  a row, the prefix is what holds an upload to its creator; after, no
  person's leaving touches the bytes.

## The rows

`app_files`, with the sync columns of every synced table:

| Column | Meaning |
| --- | --- |
| `id` | UUIDv7 from the device that created it; the server's on the web |
| `created_by_id` | the user who created it, a plain id with no foreign key |
| `name` | what a member sees, editable |
| `content_type` | from the file picked, fixed |
| `size` | in bytes, fixed |

`created_by_id` is a record, not an owner: no rule reads it but the
page, which shows the creator if the member sees them and "a former
member" if not. It has no foreign key, as `deleted_by_id` has none, so
the purge removing a user leaves their files alone, and a device takes a
file whose creator it cannot see.

`app_files_communities`, a file in a community, with the sync columns:

| Column | Meaning |
| --- | --- |
| `id` | UUIDv7 |
| `file_id` | the file, `ON DELETE CASCADE` |
| `community_id` | the community, `ON DELETE CASCADE` |
| `created_by_id` | the member who put it there, a plain id |

A unique index on `(file_id, community_id)`. Taking a file out
soft-deletes its row; putting it back revives the same row. There is no
membership column: the row is the community's, not a member's.

## Events

- **Create carries the file's communities.** The file's create event
  holds `name`, `content_type`, `size` and `communities`, a list of
  `{id, community_id}`, at least one, each a community the user is a
  live member of. The server applies the file and its rows together, so
  no file exists without a community even for a moment, and refuses the
  whole create if one is not the user's. The create waits for the bytes
  as before, `file_not_uploaded`, its rows with it.
- **Update changes `name`**, by a member of a community the file is in.
- **A file's community row is created and deleted by its own events**,
  `files_communities`, as a membership is. The server applies a delete
  and, when the file has no live row left, soft-deletes the file in the
  same transaction, with the same `deleted_at`.
- **A device never sends a file's delete.** It deletes the rows it
  sees; the server knows whether others remain. When none of the
  file's rows are left on the device, the device marks the file
  deleted there, without an event, so its bytes go by the cleanup; a
  row the server sends later puts it back.
- **Deleting a community** copies its delete onto its files' rows, as
  onto its memberships, and deletes each file that leaves with no row.

## What a user sees

Each table's `visible_to!` is one predicate, used by every read, the
pull and the backfill:

- **`app_files`**: a file with a live row in a community the user is a
  live member of.
- **`app_files_communities`**: a row whose community the user is a live
  member of. A row of a community they are not in is never sent, so
  nothing tells them a file is in it.

So a file reaches a user only through a community they are in:

- **The pull** sends the rows of both tables the predicates allow; a
  file's row is brought along by its live community row, which passed
  the check. A row taken out of a community brings no file along, since
  the user may no longer see it, and a device that does not have the
  file skips the row.
- **The backfill** of a community just joined sends its live file rows,
  with their files, after checking the user is a live member, as for
  posts.
- **The prune**, after the pull and the backfills, deletes every file on
  the device that is not visible, its rows through the foreign key, and
  its bytes at once. A file whose create has not reached the server,
  waiting or refused, stays: its bytes are nowhere else.
- **The bytes**: the signed download URL and a device's first fetch
  look the file up through the same predicate, so they refuse a file
  the user does not see.
- **The writes**: the server refuses a create into a community the user
  is not in, and a rename, a put or a take-out unless the user sees the
  file through the community concerned.

What a device already holds stays until its next sync, as for posts:
leaving cuts the user off on the server at once, and the next prune
removes the copies. Bytes saved elsewhere cannot be taken back.

## The bucket

- A file's key is `files/<created_by_id>/<file id>`. The server signs
  an upload only under the user's own prefix, so only the creator
  writes a pending file's bytes, and only the creator's create, under
  that prefix, finds them. A row's key keeps its creator's id after
  they leave and after the purge removes their user.
- The sweep deletes a key no row names, a day old, and a deleted row's
  bytes after the purge, as before.
- No bytes are deleted by person: deleting an account deletes nothing
  under its prefix at once, since files there may still be in other
  communities; a file deleted with the account goes by the sweep.

## Deleting an account

Their files are not deleted. Their memberships end as a leave does,
which leaves files where they are; a community where they were the only
member is deleted, their `Personal` one at least, and with it every file
only it had. A file in a community with other members stays, its
`created_by_id` naming a user the purge removes 90 days later.

## The pages

- **New**: pick a file, name it, and pick its communities among the
  user's, as the post form picks. The current community is picked to
  start, the one the person works in: their first by name until they
  choose one, so `Personal` for someone in no other.
- **Edit**: rename, and change its communities among the user's.
- **Show and index**: the file's communities the user is in, and its
  creator.
- **Delete**: takes the file out of the user's communities.

Trial users still have no files. A local user's files work the same on
their device, in their communities there, and never sync.

## Checksums

A file's bytes are named by a SHA-256 checksum, so the server knows the
bytes in the bucket are the bytes the person picked, and a device can
tell a file it already sees.

- **`sha256` on `app_files`**, lowercase hex, set by create and never
  changed, as the bytes never change.
- **The client computes it**, where the bytes already are: the app in
  Rust, streaming the file in a blocking task; the web page with
  `crypto.subtle.digest`, in `svelte/src/lib/core/`.
- **The bucket checks it.** The server signs the checksum into the PUT
  URL, `x-amz-checksum-sha256`, so the bucket refuses other bytes, and
  on create checks the checksum the bucket holds is the row's. The
  server never reads the bytes.
- **Duplicates are told only among files the user sees.** Picking a
  file whose checksum matches a file in the user's communities offers
  to put that file in the communities chosen instead. It is a check on
  the device's own rows, offline as well; the server never answers
  whether a checksum exists anywhere.

Bytes stored once for many files is not built; how to build it safely
is in `docs/suggestions/files.md`.

## Build order

Three phases, each built and verified before the next.

**Phase 1, communities, built:**

1. Migrations: `app_files` with `created_by_id` and no owner, and
   `app_files_communities`, both written as created this way.
2. `core/src/data/files_communities/` and the new predicates in
   `core/src/data/files/`.
3. `core/src/app/files_communities.rs`, and `files` creating with its
   communities and deleted with its last row.
4. Sync: the pull's order, the backfill, the prune.
5. The key, the sweep and account deletion.
6. Routes, Tauri commands, `svelte/src/lib/core/`, the pages.
7. Tests for every way in: a user in another community, and a creator
   who left, get no file from the pull, the backfill, the community
   rows or the download URL, and cannot write to it.

**Phase 2, the checksum:** the `sha256` column, computed by the app and
the page, signed into the upload and checked on create. First confirm
the dev bucket, Garage, refuses bytes that do not match a signed
`x-amz-checksum-sha256` and answers the checksum it holds.

**Phase 3, duplicates:** picking a file the user already sees offers
it, among all their communities and among those chosen.
