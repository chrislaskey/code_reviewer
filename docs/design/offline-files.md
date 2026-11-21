# Offline files

A file a person adds with the network off: kept on the device, tracked
by a row that syncs like a post, and uploaded to object storage when the
device is next online. The example resource is `files`, a generic
resource like `posts`, which a person can create, update and delete.

This is a second pattern beside `uploads.md`, not a replacement. A
user's picture stays as it is: online only, a column on the row it
belongs to, the page sending the bytes. Which pattern an application
picks for a given file is its own choice.

**Status: the record of the discussion, every topic settled.** The
decisions on their own are `offline-files-decisions.md`, which is the
one to build from. Each numbered topic here holds the proposal, the
alternatives weighed and how it was settled.

## Settled

- **One folder per account for every file's bytes**, uploaded or not
  (1a). A file's path depends only on its id, so uploading changes
  nothing on disk. The folder is in the app's data directory, never the
  OS cache directory, since a file not yet uploaded is the only copy.
- **The bytes never change** (2a). Create fixes them, update changes
  `name` only, replacing content is adding a new file.
- **Other devices fetch a file on first use, then keep it** (6a).
- **The device that added a file keeps its copy after the upload** (7a).
- **The device stores nothing about uploads** (5e). A pending create
  means "not uploaded", and each sync asks the server again.
- **A file is waiting, refused or synced** (topic 9). Waiting retries on
  its own; refused is on the conflicts page with Try again and Let it
  go; both ways of removing a file are local.
- **A limit of 30 attempts, on every event in the queue** (9d).
  `app_events` gains `attempts`, counted only when the server rejects
  the event or the bucket refuses the PUT; no network, a timeout,
  working offline and a failing server or bucket count nothing. At the
  limit the event is refused; Try again sets the count to 0.
- **One key per file, `files/<owner id>/<file id>`, worked out from the
  row** (5f). No `storage_key` column and no rule about an upload's
  age. A rare race with the sweep is accepted and written down, see
  "The key and the sweep".
- **Deleting a file whose create has not been applied is local only**
  (5d): the row, its events and the bytes go from the device and
  nothing is pushed.
- **Only its owner sees a file** (2b). Communities come later, by the
  join table pattern posts use.
- **Files are private in the bucket** (2c). The server signs a
  short-lived download URL for someone allowed to see the file.
- **The bytes are sent inside the sync, after the pull** (5a): ask the
  server about waiting files, push with them held back, pull, backfill,
  prune, then send the bytes and push those files' events. One
  function, `bytes::upload_waiting`, which an application with large
  files can move to a task of its own. The PUT has a longer timeout
  than the sync's other requests.
- **No device-only table** (3a). Whether a file's bytes are on this
  disk is whether the file exists at its path.
- **A file is picked with the page's `<input type="file">`** (4a), the
  same on the web and in the app, where a command takes the bytes as
  its raw body. No picker plugin.
- **A page embeds a file by one URL**, `filesContentUrl(id)`, put
  straight into `<img src>` (6b): a custom protocol in the app, serving
  the local copy or fetching it first; on the web
  `GET /api/files/:id/content`, a redirect to a signed download URL
  that the browser may cache. Anything else done with a file, copying
  it on a click say, is a separate action and does not go through the
  `<img>`.
- **Animations can be turned off, in the first version** (6d). One
  component, `FileImage`, shows the `<img>` or a still of its first
  frame. Animated unless the system asks for reduced motion; the
  person's own choice, once made, wins either way.
- **The animation choice is kept as the theme is** (6e), in the page's
  `localStorage` as `app:animations`, the same in the app and on the
  web.
- **Any type of file, up to 25 MB** (2d), the limit to adjust after
  real use. The server checks the size
  only; the content type is the device's word. Image types are
  embedded, anything else is listed by name.
- **The pages mirror posts**: an index with a small embedded image per
  image file, its name and its state; new, picking a file and naming
  it, the file's own name to start; edit, renaming; show, the file
  embedded at full size with the animation toggle.
- **Open is the show page rendering the file** (10a), on every
  platform: images, animated ones included, plain text and PDF. Every
  other type shows its name and size. More types are an application's
  to add.
- **The index lists, the show page downloads.** A click on a file in
  the index opens its show page; Download is a button there and
  nowhere else.
- **One folder per account** (1b): `accounts/<user_id>/app.db` and
  `accounts/<user_id>/files/`, so removing an account removes one
  folder. This moves the database, so `device_accounts.rs`, the paths
  in `device.db` and `bin/reset` change with it.
- **A file's name on disk is its bare id** (1c).
- **No removing of cached files to save space yet** (6c).
- **The device's cleanup, at the end of each sync** (8a): a file whose
  row is deleted is removed; a file with no row is removed once it is
  a day old.
- **The endpoints are beside the resource** (5b):
  `POST /api/files/uploads`, where a device sends its id and the web
  lets the server make one, and a `GET` under `/api/files/:id/` that
  redirects to a signed download URL, `/content`.
- **One PUT per file, started over if cut off** (5c). **No checksum**
  (2e); the server checks the size.
- **`files/` joins the server's sweep** (topic 8): a file in the bucket
  with no row, a day old, is deleted. A deleted row keeps its file for
  the purge's 90 days. Deleting an account deletes its prefix at once.
- **A refused file has no "save a copy"** (9a); it still shows on its
  device. Its state is on the files index and it is on the conflicts
  page (9b). **A name edited on two devices merges as a post's body**
  (9c). **No Download on phones yet** (10b).

The "Offline, later" sketch in `uploads.md` is ideas not yet examined,
and decides nothing here.

## What exists today that this leans on

- Every write is an event, applied at once locally and queued on a
  device; the server merges by `version` (`offline-sync.md`).
- The server signs a PUT URL and the bytes go straight to the bucket;
  nothing points at a file until the server has seen it there; files
  nobody points at go by a sweep (`uploads.md`).
- A device holds a data directory with `device.db` and one database per
  account, `accounts/<user_id>.db` (`device-accounts.md`).
- `uploads.md` has a short "Offline, later" sketch for avatars. This
  doc works that sketch out for a resource of its own, and differs from
  it in places, noted below.

## Proposed, to discuss

### 1. Where a file lives on a device

The question raised first: do files not yet uploaded and files cached
from the server live in the same place?

**Proposal: yes, one folder per account, one path per file.**

```
<data directory>/
  device.db
  accounts/
    <user_id>/
      app.db
      files/
        <file id>
```

- A file's path depends only on its id. Uploading changes nothing on
  disk: the copy the person added becomes the cached copy without a
  move. A move would be one more step that can fail halfway between the
  database and the disk.
- The pages and the Rust code read one path, whatever the file's state.
- Removing an account from the device removes one database and one
  folder.

What does differ between the two kinds is how careful we must be, and
that is a matter of state, not of place:

- A file not yet uploaded is **the only copy**. It must never be
  removed to save space, and it must not sit anywhere the operating
  system clears on its own. So the folder is in the app's data
  directory, never the OS cache directory (iOS and Android empty cache
  directories when storage is short).
- A file the server has is **a copy we can fetch again**. It may be
  removed at any time.

The alternative is two folders: `pending/` in the data directory and
`cache/` in the OS cache directory. The OS could then reclaim cached
files itself and phone backups would skip them. The cost is the move at
upload time and two places for every reader to look. If backup size
matters later, the one folder can be marked as left out of backups once
its files are uploaded, without changing the layout.

Open:

- 1a. Settled: one folder.
- 1b. Settled: a folder per account.
- 1c. Settled: the bare id.

### 2. What a file row is

**Proposal:** a table `app_files`, synced like `app_posts`.

| Column | Meaning |
| --- | --- |
| `id` | UUIDv7 from the device that added it |
| `user_id` | the owner, as on a post |
| `name` | the name the person sees, editable |
| `content_type` | from the file picked, fixed |
| `size` | in bytes, fixed |
| `version`, `last_synced_at`, `deleted_at`, `deleted_by_id`, `inserted_at`, `updated_at` | as on every synced table |

- **The bytes never change.** Create fixes them; update changes `name`
  only; delete is the soft delete. Replacing the content is adding a
  new file. This is what keeps the rest small: a cached copy is never
  stale, two devices never merge two contents, and the key needs no
  history.
- **The storage key is not a column.** It is `files/<owner id>/<file id>`,
  worked out from the row. The device names the key by naming the id,
  and the server only checks the prefix is the user's.
- Events: create may set `name`, `content_type`, `size`; update may
  change `name`, `deleted_at`.

Open:

- 2a. Settled: the bytes are fixed.
- 2b. Settled: the owner only.
- 2c. Settled: private.
- 2d. Settled: any type, up to 25 MB, one constant to adjust after
  real use. Rust sends the file from disk as a stream, never read
  whole into memory, so raising the limit later is not a rewrite.
- 2e. Settled: no checksum.

### 3. What the device knows that the server does not

Two facts are true only on one device: whether this file still needs
uploading, and whether its bytes are on this disk. They cannot be
columns of `app_files`, since a pulled row replaces the device's.

**Proposal: no extra table; both are read from what is already there.**

- Needs uploading: the row's create event is still pending in
  `app_events`.
- Bytes on this disk: the file exists at its path.

The alternative is a device-only table (`files_local`: file id,
uploaded at, last opened at). It would be needed for removing the least
recently opened files when space runs short, which the first version
does not do.

Open:

- 3a. Settled: derive.

### 4. Adding a file offline

1. The person picks a file. Rust checks the size and the name with the
   same rules the server runs, so a wrong file is told at once.
2. Rust copies the file into the account's folder under a new id.
3. `files::create` writes the row and queues the create event, in one
   transaction, as for a post.
4. The page shows the file from the local copy.

Step 2 comes before step 3, so a crash in between leaves a file with no
row, which the device's sweep removes (topic 8), and never a row with no
file.

Open:

- 4a. Settled: the page's `<input type="file">`.

### 5. The exchange with the server

Agreed in outline: no separate upload queue. The details below are
still to settle.

**How the device knows a file is not uploaded.** It does not look at
the bucket; it holds no keys. A file needs uploading exactly when its
create event is still pending in `app_events`. That event
leaves the queue only when the server answers the create `applied`, and
the server answers that only after seeing the file in the bucket. So the pending
create is the whole record, and nothing else is stored on the device.

**One sync, step by step.** The table shows one file's path; in the
sync, steps 1 to 3 come first, then the push of everything not held
back, the pull, the backfill and the prune, then steps 4 to 7 (5a):

| Step | Who | What |
| --- | --- | --- |
| 1 | device | reads the pending create events on `app_files` |
| 2 | device to server | for each, asks for an upload: `{id, content_type, size}` |
| 3 | server | checks the size against the limit, then looks in the bucket at `files/<owner id>/<file id>` (one HEAD request). A file there of the right size: answers `uploaded`. Otherwise: answers a signed PUT URL |
| 4 | device | for a URL, sends the bytes from disk, from Rust. A success counts the file as uploaded |
| 5 | device | pushes every pending event, except those for a file with a pending create that steps 3 and 4 did not count as uploaded in this sync. Its later events, a rename say, wait with it |
| 6 | server, `files::apply_event` | on create, looks in the bucket again, the same check as step 3. There: the row is written, `applied`. Not there: `file_not_uploaded` |
| 7 | device | applies the answers as for any row; the local copy stays |

- **The list of what to hold back lives in memory for one sync.** The
  next sync asks again for each create still pending. That is one small
  request per file waiting, and files wait only until their first
  successful sync.
- **The check on the bucket is the server's, made twice.** In
  step 3 it saves sending bytes the bucket already has. In step 6 it is
  the rule: the server never writes a row whose file it does not have.
  The device's own belief is never trusted and never stored.
- **`file_not_uploaded` is not a refusal.** It follows today's rule for
  a rejection without a server row: the event stays pending with the
  reason, and the next sync's step 3 finds the file missing and uploads
  it again. No person is involved.
- Every step can run again. A sync cut off after step 4 asks again,
  hears `uploaded`, and pushes.
- A failed upload holds back only that file's events. Posts and other
  files push as usual.
- The `Transport` trait gains the request of step 2 and the sending of
  step 4, so the in-process transport for tests puts the bytes into the
  in-memory bucket and the exchange is tested without HTTP.
- On the web there is no queue: the page asks for the URL, sends the
  bytes, then creates the row, in that order, as the avatar does.

#### The key and the sweep

Settled: one key per file, and the simple check. What that leaves:

The server's sweep deletes files no row points at once they are a day
old. A file uploaded whose push never happened is such a file.

- **The usual case mends itself.** Uploaded on Monday, swept on
  Tuesday, the device back on Wednesday: the server finds no file,
  answers a PUT URL, the device sends the bytes again and pushes.
- **The accepted gap.** The device comes back in the few seconds a
  sweep is running, after the sweep read the rows and before it deletes
  that key. The server sees the file and writes the row; the sweep then
  deletes the file. It needs a file left without a row for over a day
  and a sync landing inside a daily job's few seconds.
- **What the person sees then.** The file is listed. It opens on the
  device that added it, which kept its copy, and not on others. They
  delete it and add it again.

Closing the gap fully needs a new key per upload and a `storage_key`
column the server sets; not worth it for something this rare whose cost
is adding a file again. If it ever shows up, the cheap first step is
the sweep checking for a row once more just before each delete.

What a person sees while this runs: the files page marks a file as
"Waiting to upload" while its create is pending, with the last error
under it if an attempt failed, and the nav's pending count includes it.

Open:

- 5a. Where the bytes are sent from. Three ways:

  | | How | Cost |
  | --- | --- | --- |
  | A | inside the sync, before the push | simplest: one push. Everything waits behind the upload, this sync's posts and pull included |
  | B | inside the sync, after the pull: ask about waiting files, push with them held back, pull, backfill, prune, then send the bytes and push those files' events | a second push. This sync's posts and pulled rows are not delayed; the lock is still held while bytes are sent, so the next sync waits |
  | C | a task of its own with its own lock; the sync only asks the server which files are uploaded and holds back the rest; a finished upload starts a sync | nothing waits on an upload. A second lock, which switching accounts must also wait for, and attempts counted in two places |

  How long the lock is held in A and B, for one 25 MB file: about 10
  seconds at 20 Mbps up, 40 at 5 Mbps, over 3 minutes at 1 Mbps. Several
  files add up.

  Settled: B. The general case is small files; C is an upgrade an
  application makes for itself.
- 5b. Settled: beside the resource, not under `/api/sync/`. Open: the
  download's name, `/content` or `/url`; it answers a redirect to the
  bytes for `<img src>`, not a URL in JSON.
  - `POST /api/files/uploads`, `{id, content_type, size}`: answers
    `uploaded` or a signed PUT URL, good for 10 minutes as an avatar's
    is. A device sends the id it made; the web sends none and the
    server makes one and answers it. The file has no row on the server
    yet, so what the server holds the user to is the key's prefix,
    their own.
  - `GET /api/files/:id/content`: the redirect to a signed download
    URL, good for an hour, for a user who sees the row.
  - `GET`, `POST`, `PUT` and `DELETE` on `/api/files` as for posts.
- 5c. Settled: one PUT, started over if cut off.
- 5d. Settled: local only. This differs from a post, whose create and
  delete both push.
- 5e. Settled: the device asks again on every sync.
- 5f. Settled: one key per file.

### 6. Reading a file, and the cache

A pulled row arrives without bytes.

**Proposal: fetch on first use, then keep.** When a page asks for a
file, Rust serves the local copy if there is one; otherwise it asks the
server for a download URL, saves the bytes to the file's path, and
serves that. Offline with no local copy, the page says the file is not
on this device.

- The page reaches the bytes through one app URL per file, a custom
  protocol handled in Rust (`files://<id>`, say), so an `<img>` or a
  link works and the page never learns whether the file was cached.
- On the web, the same core function answers the server's URL.

The alternative is fetching every file as its row is pulled, which
makes everything available offline and costs disk and bandwidth for
files never opened. This is the choice the request names as varying by
application; the proposal is the cheaper one, and a comment in the code
says where to change it.

Open:

- 6a. Settled: on first use.
- 6b. How a page reaches the bytes. The main use is files embedded in
  a page beside its other content, images above all, not opened or
  downloaded one by one. Proposal: one URL per file on both sides, from
  `filesContentUrl(id)`, put straight into `<img src>`.
  - In the app, a custom protocol handled in Rust, which serves the
    local copy with the row's content type, or fetches it first when it
    is missing. A file that cannot be had, offline and never fetched,
    is a 404 and the page shows a placeholder.
  - On the web, `GET /api/files/:id/content`, which checks the user and
    redirects to a signed download URL. The redirect is answered as
    cacheable by that browser for a little less than the URL's
    lifetime, so a page of images shown again asks for nothing; the
    bytes never change, so a cached image is never wrong.

  An animated GIF in an `<img>` plays on its own in every webview.
  The alternatives in the app: Tauri's asset protocol with the folder
  allowed in the config, less code but it cannot fetch a missing file;
  or a command that returns the bytes for the page to wrap in a blob
  URL, which holds every embedded file in the page's memory, the
  wrong fit for a page of images.
- 6d. Settled: in the first version. A browser has no switch for a
  GIF in an `<img>`, so `FileImage`, used wherever a file is embedded,
  renders the `<img>`, or with animations off draws the image's first
  frame onto a canvas. Presentation only. The choice has three values:
  follow the system (the start), on, off. Following the system means
  off when it asks for reduced motion and on otherwise; the toggle sets
  on or off, which holds whatever the system says.
- 6e. Settled: in `localStorage`, as the theme.
- 6c. Settled: none in the first version.

### 7. After the upload: keep or remove the local copy

**Proposal: keep.** With one folder it costs nothing to do, and the
person who added a file is the one most likely to open it next. An
application that wants the other behaviour removes the copy when the
create is answered `applied`, a single place in `sync::run`.

Open:

- 7a. Settled: keep.

### 8. Deleting

- **A delete is the soft delete**, synced like any edit.
- **In the bucket**, the file stays while its row exists, deleted or
  not, and goes by the sweep after the purge takes the row: `files/`
  joins `avatars/` as a prefix the sweep reads, matching a key's id
  against `app_files`.
- **On a device**, a cleanup of the same kind at the end of each sync,
  over the account's folder:
  - a file whose row is deleted is removed;
  - a file with no row at all is removed once it is a day old.

  The day is for a file being added: its bytes are on disk a moment
  before its row is written, and a sync running in that moment must
  not take them. The one rule covers a delete, a prune, a resync that
  dropped the rows, and a copy left by a crash before its row was
  written.
- A file being fetched is written under a temporary name and renamed
  when whole, so a half-fetched file is never shown.

Open:

- 8a. Settled: at once.

### 9. When a file cannot be uploaded, and never being stuck

Agreed in outline: a refused file is not lost. The mechanism below is
still to settle.

Today a rejected create deletes the event and the row it made. For a
file that would delete the only copy of something the person added, so
files do not follow that rule. A file on a device is always in exactly
one of three states, and each has a way out.

| State | How it gets there | What happens on its own | What the person can do |
| --- | --- | --- | --- |
| Waiting | added; the create is pending | every sync tries the upload and the push again | delete the file on the files page |
| Refused | the server answered that it will not take this file | nothing; no retry | Try again, or Let it go, on the conflicts page |
| Synced | the create was applied | syncs like a post | rename, delete |

**Waiting retries without anyone asking.** These are the failures that
may pass: no network, the PUT failed, the signed URL expired,
`file_not_uploaded`. The event stays pending and the next sync, at most
half a minute later while the app is in front, tries again from step 1.
There is no count of attempts and no giving up.

**Refused is for answers that will not change by trying again.** The
server will not sign an upload or apply the create: the file is larger
than the limit, the name is not valid, or a rule an application adds
(a quota, say). The device checks the same rules when the file is
picked, so this happens only when the device and the server disagree:
an older app, or a limit lowered on the server. One more case is found
by the device itself: the bytes are gone from its disk, so there is
nothing to send.

A refused file's create is kept as a conflict with nothing on the
server's side, its row and its bytes stay on the device, and the file
still opens there. The conflicts page shows it with the server's
reason and two buttons:

- **Try again** makes the create pending again, so the next sync runs
  the whole exchange. Useful after renaming the file, updating the app,
  or the server's limit changing. If the server refuses again, the file
  is back on the conflicts page with the new reason.
- **Let it go** removes the event, the row and the bytes from the
  device. Nothing of it ever reached the server, so nothing else is
  left to undo.

**Nothing is permanently stuck**, because removal never depends on the
server: deleting a waiting file (5d) and letting a refused one go are
both local, work offline, and work whatever the server says. The one
thing that cannot be forced is success: a file over the limit is never
accepted, and the person's choices are to let it go or keep it on this
device only, where it stays listed as refused.

The conflicts page and the pending changes list read only posts today
(`conflicts.rs`, `pending_changes.rs`). Both learn a second table: a
file is shown by its name, a refused create with its reason and Try
again in place of Reapply.

A rename made while the file waits or is refused edits the row and
queues an update behind the create, and travels with it.

#### A limit on attempts

Today an event the server rejects without a row stays pending and is
pushed again on every sync, for ever. That is true of a post's event
as much as a file's, so a limit belongs to the queue, not to files.

Settled: `app_events` gains `attempts`, and the limit is 30.

What counts as an attempt is the server saying no to this event:

| What happened | Counts |
| --- | --- |
| no network, a timeout, the app set to work offline | no |
| the server or the bucket answered that it is failing (a 5xx) | no |
| the server rejected the event and it stays pending, `file_not_uploaded` included | yes |
| the bucket refused the PUT (a 4xx: a bad signature, a wrong content type) | yes |

- An outage is the server or the bucket failing, so it counts nothing
  however long it lasts, and the limit never has to be sized against
  one. That is why a plain limit is enough, with no longer wait after
  each failure and no second column.
- What does count is an answer about this event that will likely be
  the same next time, which is the endless loop the limit is for. At a
  sync every half minute, 30 is a quarter of an hour of the app in
  front.
- At the limit the event becomes refused, with the last reason and the
  number of tries. Try again sets `attempts` to 0 and makes it pending.
- The limit covers every event in the queue, a post's too.

Open:

- 9d. Settled.
- 9a. Should the conflicts page also offer "Save a copy" before
  letting a refused file go, or is opening the file from the app
  enough? Proposal: not in the first version.
- 9b. Should a refused file appear on the files page as well, marked,
  or only on the conflicts page? Proposal: both, since the row exists.
- 9c. A name edited on two devices merges like a post's body; nothing
  new. Agreed?

### 10. Opening and downloading a file

With any type allowed, a file that is not an image needs to be looked
at somehow, or it can be added and never seen.

**Open means the app shows the file itself, as a browser does for a
link**, on every platform. Not the system's program for the type.

A webview is not a whole browser: it has no tabs, no back button and
no download bar, so sending the window to the file's URL would leave
the person outside the app with no way back. So the show page is where
a file opens, and it renders by type, the same component on the web
and in the apps, fed by the same content URL:

| Type | Shown with | Where it works |
| --- | --- | --- |
| image | `<img>`, through `FileImage` | everywhere |
| plain text | a sandboxed `<iframe>` | everywhere |
| PDF | a sandboxed `<iframe>` | browsers; expected on macOS, iOS and Windows, whose webviews have a PDF viewer; not expected on Android or Linux, whose webviews have none. To verify on each while building |
| anything else, or a type the platform cannot show | the name, the size, and Download | |

- **Only listed types are rendered.** An HTML or SVG file is a
  program; it is never loaded as a document, and the protocol answers
  every file with `nosniff` so the webview does not guess otherwise.
- **Audio and video are left for later**: WebKit asks for a media file
  in ranges, which the protocol would have to answer.

**Download** saves a copy outside the app:

| | How |
| --- | --- |
| Web | the content URL asking for an attachment, so the browser saves it under the file's name. To verify: that the signed URL can ask the bucket for that with `object_store` |
| Desktop app | the local copy copied to the Downloads folder under its name |
| Phone app | later: an app's files are closed to other apps and the way out is the system's share sheet, which needs a plugin |

Open:

- 10a. Settled: rendering on the show page by type.
- 10b. Download on the web and the desktop in the first version, the
  button absent on phones until the share sheet?
- 10c. Settled: images, plain text and PDF; the rest later.

### 11. What is left out of the first version

Communities for files; replacing content; copying an image to the
clipboard on a click, for which the quick search popup on the branch
`v2-elixir-and-rust` is the pattern to start from; multipart and resumable
uploads; uploads while the app is in the background; the size of the cached
files shown with a "Remove cached files" button, which removes every
file the server has and leaves those waiting or refused; previews and thumbnails; checksums; attaching a
file to a post. Each would go to `docs/suggestions/` once the scope is
agreed.

## Build order, proposed

Each phase works and is tested on its own.

| Phase | Builds | Proves |
| --- | --- | --- |
| 1 | `app_files`, `files` in core with events, the routes and commands, the pages, online only on the web: URL, PUT, create | a file added on the web is listed, renamed and deleted; a create without its file in the bucket is refused |
| 2 | the account's folder, adding a file on a device, showing it from disk | a file added offline is listed and opens with the network off |
| 3 | the upload step in `sync::run`, the transport, events held back | a file added offline reaches the bucket and the web; a failed upload holds only that file's events; a sync cut off finishes on the next |
| 4 | reading on another device: download on first use, the protocol | a file added on one device opens on another, then opens offline |
| 5 | deleting: the device's sweep, `files/` in the server's sweep, a refused file kept as a conflict | no file without a row on either side; a refused file is not lost |

## Questions, in the order worth discussing

1. 10b: download on phones.
2. The rest: 1b, 1c, 2e, 5b, 5c, 6c, 8a, 9a, 9b, 9c.
