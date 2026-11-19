# Uploads

How a file a person picks reaches object storage, how a row comes to
point at it, and how a file nobody points at goes away. The first file
is a user's picture, `app_users.avatar_url`; the rules here are meant to
hold for the next ones, a post's attachments say. Online only: a file
added with the network off is another pattern, the `files` resource,
see `offline-files-decisions.md`.

## Decisions

- **A file never goes through the server.** The server holds the
  bucket's keys and signs a URL for one PUT; the person's device sends
  the bytes to the bucket. `object_store` signs, so the bucket is any
  S3 API: Garage in dev (`bin/storage`), AWS, R2 or Tigris in
  production.
- **The server names the key**, `avatars/<user id>/<uuid v7>.<ext>`:
  under the user's own prefix, so a key says whose it is and an account
  deleted is one prefix to delete, and new every time, so a file is
  never overwritten and a URL never shows another picture than it did.
- **Nothing points at a file until the server has seen it.** The row
  changes only after the server checks the file is there, no larger
  than the limit, and starts with the bytes of the image type its key
  says; a file that fails is deleted at once.
- **A row points at a file through its public URL**, which the server
  makes from the key, `STORAGE_PUBLIC_URL` and the key. Pictures are
  read by anyone holding the URL, which only says what a v7 UUID makes
  hard to guess; anything private would be read through signed GET URLs
  instead.
- **Files go after rows, never before**, by a sweep, see "Deleting".

## The flow

| Step | Who | What |
| --- | --- | --- |
| 1 | page | the person picks a file; `accept` filters the picker, nothing else is checked here |
| 2 | server, `avatars::create_upload` | `AvatarUploadNew {content_type, size}` checked with garde: a PNG, JPEG, GIF or WebP, 1 byte to 5 MB, else 422 with the fields' messages; answers `{upload_url, content_type, key}`, the URL good for 10 minutes |
| 3 | page, `putFile` | PUT to `upload_url` with that `Content-Type`, through `XMLHttpRequest` for the progress bar |
| 4 | server, `avatars::update` | `{key}`: the user's prefix, the file there, its size, its first bytes; then `avatar_url` set as an event in the server's log, the version up one |
| 5 | devices | pull the user's row like any other |

In the app, steps 2 and 4 are commands that ask the server over the sync
transport's HTTP client (`avatars::create_upload_on_device`,
`update_on_device`); step 2 checks the fields on the device first, so a
wrong file is told without the network, and step 4 writes the row the
server answers at once instead of at the next pull.

The URL is signed for its content type, so the bucket refuses a PUT
with another. `object_store` will not sign `Content-Length` into a PUT
URL, so the size is only checked in step 4: someone holding a URL for
10 minutes can store a file larger than the limit until step 4 or the
sweep deletes it. A bucket quota bounds that (`garage bucket
set-quotas`); a presigned POST policy with `content-length-range` would
stop it at the bucket, which neither `object_store` nor `aws-sdk-s3`
signs, so it would be our own code.

## When something fails

| Fails | What is left | What happens |
| --- | --- | --- |
| step 2: a wrong file | nothing | the fields' messages under the input |
| step 3: the PUT, the network, the tab closed | nothing; a single PUT is all or nothing | the page says to try again; the URL expires; a new try is a new key |
| between 3 and 4: the page never confirms | a file nobody points at | the sweep |
| step 4: not the image it said, too large | nothing; the file is deleted there | the message on the page |
| step 4: the database write | a file nobody points at | the page says so; the sweep |
| a picture replaced | the old file, nobody points at it | the sweep |

Every row that is left wrong is one we never wrote, and every file that
is left is one nobody points at, which is the one leftover that is
harmless and easy to find.

## Deleting, the sweep

A file cannot be deleted in the database's transaction: there is no
transaction across the database and the bucket. Of the two orders, file
first then row leaves a row pointing at nothing if the row's delete
fails, a broken picture; row first then file leaves a file nobody
points at if the file's delete fails, which nobody sees. So rows go
first, and files after, by reconciliation rather than bookkeeping:

- **The sweep**, `avatars::delete_unreferenced(deployment, storage,
  now)`: list the bucket under `avatars/`, and delete every file that no
  user points at, deleted or not, and that is older than a day. Step 4
  refuses a key made more than an hour before, from its v7 UUID, so a
  file the sweep may take can never become someone's picture after the
  sweep read the rows. A row's URL is matched by the key at its end, so
  moving the bucket behind a CDN never makes a picture look unused.
- **It runs daily, after the purge**, as a job, see
  `docs/design/jobs.md`.
- **Every kind of leftover is the same case.** An upload never
  confirmed, a write that failed, a picture replaced, a row purged: the
  sweep looks at what the rows point at now, not at what happened, so
  it needs no table of uploads and no record of what was deleted, and a
  run that fails halfway is finished by the next.
- **Soft deletes keep their files.** A row deleted is still a row for
  90 days, and it still points at its file, so whatever brings it back,
  a rejoin bringing back posts, brings back its pictures. The purge
  removes the row after 90 days and the next sweep its file.
- **Cascades need nothing more.** `on delete cascade` runs in the
  database, where no code of ours can delete a file, but the sweep does
  not need to be told: a row a cascade took no longer points at its
  file.
- **Each table with files says which column and which prefix**, so the
  sweep reads every table's references; `avatar_url` and `avatars/` are
  the first, `avatars::delete_unreferenced`. `app_files` and `files/`
  are the second, `files::delete_unreferenced`, where a file's key is
  worked out from its row.
- **Deleting an account** deletes its prefixes at once, without waiting
  for the sweep, since erasure is owed promptly.

Listing the bucket each day is fine into the millions of files; past
that, the bucket's inventory report or a table of uploads would replace
the listing, not the rule.

A picture replaced is swept the next day, so a device that has not
synced since shows a broken picture until it does. Keeping replaced
files longer needs to know when a file stopped being pointed at, which
is the table of uploads again; not worth it for pictures.

## Offline, later

Ideas for a picture, never examined; what was built for files added
offline is `offline-files-decisions.md`, which differs: no upload
queue, and the local copy is kept.

The app would set a picture as it writes a post: at once on the device,
the change queued and pushed. The file has to wait for the network, so:

1. The device names the key itself, `avatars/<user id>/<uuid v7>.<ext>`,
   copies the file into the account's folder, and writes `avatar_url`
   locally as an event, the page showing the local copy until it is
   uploaded.
2. An upload queue, beside the event queue, sends each file when online:
   the server signs a PUT for the key the device named, after checking
   it is under the user's prefix.
3. The push holds the event until its file is uploaded, and the server's
   `apply_event` for `app_users` runs step 4's checks, so the server
   never points at a file it does not have. That needs `app_users` in
   the push, with `avatar_url` editable by its owner and merged like a
   post's body.
4. Answered: applied deletes the local copy; a conflict, another device
   set a picture first, is kept for the person like any conflict, and
   either way the file that lost is left to the sweep; a rejection
   deletes the local copy, and its uploaded file is left to the sweep.

So the third failure case, a file uploaded and written locally whose
event the server then rejects, ends like the others: a file nobody
points at, which the sweep deletes.

## Dev

`bin/storage start` runs one Garage node under `axum/data/garage/`, set
up on every start, each step only when it is missing, so a setup that
stopped part way is finished on the next: the bucket `starter-pack`, readable by anyone
through Garage's web endpoint at `http://starter-pack.web.localhost:<port>`,
the bucket `starter-pack-private` for files, read through signed URLs
only (`offline-files-decisions.md`), CORS open to every origin for the
pages' PUTs, and a key for the server, kept there, never committed. `bin/dev` starts it with the web
server and passes the server `bin/storage env`. Production's buckets
allow only the web's origin and the app's (`tauri://localhost`,
`http://tauri.localhost`), `PUT` on both and `GET` on the private one,
which the web's Copy reads an image with, and on the private one the
`x-amz-checksum-sha256` header a file's `PUT` carries.

The URLs are `127.0.0.1` and `*.localhost`, which the desktop and the
iOS simulator reach; an Android emulator or a phone would need the
Mac's address in `STORAGE_ENDPOINT` and `STORAGE_PUBLIC_URL`.
