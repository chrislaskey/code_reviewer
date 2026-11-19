# Offline files

A file a person adds with the network off: kept on the device, tracked
by a row that syncs like a post, and uploaded to object storage when the
device is next online. The example resource is `files`, generic like
`posts`: a person adds a file, renames it, deletes it.

This is a second pattern beside `uploads.md`, which stays as it is: a
user's picture is online only, a column on its row, sent by the page.
Which pattern a file follows is the application's choice. How these
decisions were reached, with the alternatives, is `offline-files.md`.
Built as written, in `core/src/app/files.rs`; what was left out is
`docs/suggestions/files.md`.

## Decisions

- **A file is a row, `app_files`, and its bytes.** The row syncs like a
  post, through events and versions. The bytes never change: create
  fixes them, update changes `name` only, replacing content is adding a
  new file. So a copy on a device is never out of date.
- **A file belongs to its communities.** A live member of one sees it,
  and it lives as long as it is in one, see `files-communities.md`.
- **Any type, up to 25 MB**, one constant. The bytes are checked by
  their SHA-256, which the row holds: the bucket refuses other bytes,
  and the server checks the size and the checksum the bucket holds,
  see "Checksums" in `files-communities.md`. The content type is the
  device's word.
- **Private, in a bucket of their own**, `STORAGE_PRIVATE_BUCKET`, at
  `files/<creator id>/<file id>`, a key worked out from the row and
  never stored. Reading is through a download URL the server signs for a
  user who sees the row. Pictures stay in the public bucket,
  `STORAGE_BUCKET`: public or private is a whole bucket's on most
  stores.
- **On a device, one folder per account holds every file's bytes**,
  uploaded or not, each under its bare id. In the app's data directory,
  never the OS cache: a file not yet uploaded is the only copy.
- **The device stores nothing about uploads.** A file needs uploading
  exactly when its create event is still pending. Whether its bytes are
  on this disk is whether the file exists at its path.
- **The server never holds a row whose file it does not have.** It looks
  in the bucket before it applies a create.
- **A device keeps the bytes**: the one that added a file after the
  upload, another from the first time it shows the file. Nothing is
  removed on its own; "Remove cached files" at the foot of the files
  index, `files::cache_delete`, removes the copies of synced files,
  each fetched again when next shown, and leaves waiting and refused
  files, whose bytes are nowhere else.
- **A file is waiting, refused or synced**, and never stuck: waiting
  retries on its own, refused waits for the person, and removing a file
  in either state needs nothing from the server.
- **An event is tried 30 times**, every event, a post's too, counting
  only the server's no to that event.

## The row

`app_files`, with the sync columns of every synced table:

| Column | Meaning |
| --- | --- |
| `id` | UUIDv7 from the device that created it; the server's on the web |
| `created_by_id` | who created it, with no foreign key, see `files-communities.md` |
| `name` | what the person sees, editable |
| `content_type` | from the file picked, fixed |
| `size` | bytes, fixed |

An event may set `name`, `content_type`, `size` and `communities`, the
rows of its first communities, on create, and change `name` on update;
a file is deleted when it is taken out of its last community. A name edited on two devices
merges as a post's body does.

## On a device

```
<data directory>/
  device.db
  accounts/
    <user_id>/
      app.db
      files/
        <file id>
```

Removing an account removes its folder. A sign-out keeps it, as it
keeps the database.

**Adding a file.** The page's `<input type="file">`, the same on the
web and in the app, where a command takes the bytes as its raw body:

1. Rust checks the size and the name by the rules the server runs, and
   works out the bytes' SHA-256 in a blocking task.
2. Rust writes the bytes into the account's folder under a new id.
3. `files::create` writes the row and queues the create event, in one
   transaction.

Bytes before row, so a crash in between leaves a file without a row,
which the cleanup removes, and never a row without a file.

## The exchange, in `sync::run`

| Step | Who | What |
| --- | --- | --- |
| 1 | device, `files::ask_about_waiting` | for each pending create on `app_files`, the oldest `files::FILES_PER_SYNC` (100), asks `POST /api/files/upload` with `{id, content_type, size, sha256}` |
| 2 | server | checks the size; looks at `files/<creator id>/<file id>`. There, of that size and with that checksum: `uploaded`. Otherwise a PUT URL signed for the content type and the checksum, good for 10 minutes |
| 3 | device | pushes every pending event except those of a file not answered `uploaded`, `files::held_back`; a rename waits with its create |
| 4 | device | pulls, backfills, prunes, as ever |
| 5 | device, `files::upload_waiting` | sends each waiting file's bytes from disk to its URL, as a stream, with a longer timeout than the sync's requests |
| 6 | device | pushes the events of the files just sent |
| 7 | server, `files::apply_event` | on create, looks in the bucket again: there, the row is written; not there, `file_not_uploaded`, and the event stays pending for the next sync |

- What to hold back is worked out in each sync and kept nowhere. Once
  the server cannot be reached the rest of the files are not asked
  about, so a device offline waits on one request, not one per file.
- A sync takes 100 files at most, and holds the rest back unasked, so
  every URL is used within its 10 minutes and the lock is not held for
  thousands of files. One that sent or pushed something and left files
  starts the next sync at once.
- Every step can run again: a sync cut off after step 5 hears
  `uploaded` next time and pushes.
- A file that fails holds back only its own events.
- The bytes go last, so a sync's posts and pulled rows never wait on
  them. The sync lock is held while they go; an application with large
  files moves `files::upload_waiting` to a task of its own, which
  changes who calls it and no data.
- One PUT per file, started over if cut off, carrying the checksum in
  `x-amz-checksum-sha256`, which the bucket checks the bytes against.
- The file has no row on the server when its upload is asked for, so
  the server holds the user to the key's prefix, their own: nobody else
  writes or claims a file before its row exists.
- The `Transport` trait carries the asking, the sending and the
  fetching (`files_create_upload`, `files_upload`, `files_download`),
  so tests run the exchange in one process against the in-memory
  bucket.
- On the web there is no queue: the page asks for an upload without an
  id, the server makes one, the page sends the bytes and creates the
  row.

## Waiting, refused, synced

| State | Gets there | On its own | The person can |
| --- | --- | --- | --- |
| Waiting | added; the create is pending | every sync tries again | delete it |
| Refused | the server will not take it, its bytes are gone, or 30 tries | nothing | Try again, Let it go |
| Synced | the create was applied | syncs like a post | rename, delete |

- **Refused keeps everything.** A post's rejected create is dropped
  with its row; a file's is kept as a conflict with nothing on the
  server's side, with its row and its bytes, and the file still shows
  on that device. Refused at once: the server will not sign an upload
  for it, or rejects its create for anything but missing bytes, over
  the limit or a name not valid say; or the device finds the bytes gone
  from its disk. Refused at the limit: the tries below. A refused
  file's later events, a rename, are held back too, since the server
  never had the row.
- **Try again** makes the create pending and sets its tries to 0.
  **Let it go** removes the event, the row and the bytes.
- **Deleting a waiting file is local**: the row, its events and the
  bytes go, and nothing is pushed, since nothing reached the server.
- The files index shows each file's state, and under a waiting one the
  last error. A refused file is also on the conflicts page, by its
  name, with the reason. The conflicts page and the pending changes
  list read posts only today and learn this second table.

**What counts as a try**, `app_events.attempts`:

| What happened | Counts |
| --- | --- |
| no network, a timeout, working offline | no |
| the server or the bucket failing, a 5xx | no |
| the server rejected the event and it stays pending | yes |
| the bucket refused the PUT, a 4xx | yes |

An outage counts nothing however long, so the limit is not sized
against one. The reason is kept under the file either way, counted or
not, so the page says why a file still waits.

## Showing a file

One URL per file, `filesContentUrl(id)`, put straight into `<img src>`
or a frame:

- **In the app**, a custom protocol handled in Rust: it serves the local
  copy with the row's content type, and when there is none fetches the
  file first, `files::local_copy`, written under a temporary name and
  renamed when whole. A file that cannot be had, the server out of
  reach or the device working offline, is a 404 and the page shows a
  placeholder.
- **On the web**, `GET /api/files/:id/content` redirects to a signed
  download URL good for an hour, the redirect cacheable in that browser
  for a little less.

The index lists the files; a click opens the show page, which renders
by type, the same on every platform:

| Type | Shown with |
| --- | --- |
| image, animated or not | `FileImage` |
| plain text | a sandboxed frame |
| PDF | a frame, not sandboxed: a sandbox turns off the webview's PDF viewer |
| anything else | its name and size |

Only these types are rendered, and every answer says `nosniff`: an HTML
or SVG file is a program and is never loaded as a document. More types
are an application's to add.

**Animations.** `FileImage` shows the `<img>`, or a still of its first
frame drawn on a canvas. The choice is kept as the theme is, in
`localStorage` as `app:animations`: follow the system to start, which
is off when it asks for reduced motion, then on or off as the person
sets it, whatever the system says.

**Copy.** On the show page a click on an image copies it to the
clipboard:

| Where | What goes on the clipboard | An animated GIF |
| --- | --- | --- |
| macOS app, `tauri/src/clipboard/macos.rs` | one item: the file, its own bytes and a PNG; a TIFF too when `TIFF_RENDERING` is on, off for now | animated where GIFs are taken, a still elsewhere |
| iOS app, `tauri/src/clipboard/ios.rs` | one item: its own bytes and a PNG | the same |
| web, `filesCopy` in `http-api.js` | a PNG, the one image type browsers write | its first frame |

The web reads the bytes with `fetch`, so the private bucket's CORS
must allow `GET` from the web's origin. Windows, Linux and Android have
no Copy yet, see `docs/suggestions/files.md`.

**Download** is a button on the show page only: on the web the content
URL asking for an attachment; in the desktop app a copy into the
Downloads folder; absent on phones.

## Deleting

A delete is the soft delete, synced like any edit.

- **On a device**, a cleanup at the end of each sync, over the
  account's folder, `files::delete_unreferenced_on_device`: a file
  whose row is deleted is removed; a file with no row, or a fetch left
  half done, is removed once it is a day old. The day is for a file
  being added, whose bytes are on disk a moment before its row.
- **In the bucket**, `files/` joins `avatars/` in the server's sweep,
  `files::delete_unreferenced`: a file with no row, a day old, is
  deleted. A deleted row keeps its file for the purge's 90 days.
  Deleting an account deletes no file's bytes at once, since its files
  may still be in other communities; the sweep takes those of the files
  deleted with it. The sweep runs daily after the avatars', see
  `docs/design/jobs.md`.

**A gap accepted.** A file uploaded whose push never came is swept
after a day and uploaded again when the device returns. If the device
returns in the seconds a sweep is running, the row can be written just
before the sweep deletes the file: the file then shows on the device
that added it and not on others, and the person adds it again. Closing
this needs a new key per upload and a stored key; not worth it. The
cheap first step, if it ever shows, is the sweep looking for a row
again before each delete.

## To verify while building

- A PDF in a frame from the custom protocol, on each platform's
  webview; where it does not render, the page shows the name and size.
- The canvas still of an image served by the custom protocol.
- That the bucket honours `response-content-disposition` in the
  signed download URL, which is how Download on the web asks for an
  attachment; `object_store` signs it.
- 25 MB through the page's file input and a command's raw body on a
  phone.

## Left for later

Replacing content; multipart and resumable
uploads; uploads while the app is in the background; audio and video,
which need the protocol to answer ranges; download on phones, through
the share sheet; copying an image on the platforms other than macOS;
thumbnails; bytes stored once for many files; a file attached to a
post. See `docs/suggestions/files.md`.

## Build order

Each phase ends in a state that works, is tested and is worth keeping.

| Phase | Builds | Proves |
| --- | --- | --- |
| 1 | the migration `files_create`; `files` in core with its events and SQL; `app_files` in the push and the pull; signed download URLs; the routes; the web's pages, with `FileImage`, the animation choice and Download | on the web a file is added, shown, renamed, deleted and downloaded; a create without its file in the bucket is refused; another user neither lists it nor gets its content |
| 2 | a folder per account: `accounts/<user_id>/app.db`, `bin/reset` | the account tests pass on the new layout; removing an account removes its folder |
| 3 | adding a file on a device; the custom protocol serving the local copy; the commands; the desktop's Download | a file added offline is listed as waiting and shows with the network off; deleting it leaves nothing behind |
| 4 | the exchange: the transport, `files::upload_waiting`, events held back, the second push | a file added offline reaches the bucket and the web; a failed upload holds back only its own events; a sync cut off finishes on the next; `file_not_uploaded` is tried again |
| 5 | fetching on first use | a file added on one device shows on another, then with the network off |
| 6 | the migration `events_add_attempts`; what counts; refused; the conflicts page and the pending changes for files | 30 rejections refuse an event and an outage counts nothing; Try again starts over; Let it go removes the bytes; a refused file is not lost; a name edited on two devices is a conflict |
| 7 | the device's cleanup; `files/` in the server's sweep; `offline-sync.md` and `uploads.md` brought up to date; `docs/suggestions/files.md` | after a day no file is without a row on either side; a deleted row's bytes leave the device |
