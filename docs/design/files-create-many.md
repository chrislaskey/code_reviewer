# Creating many files at once

A person picks many files and creates them in one go: they review the
list, choose the communities once for all of them, decide what to do
with each duplicate, and watch each file go. Built on the single-file
create of `files-communities.md` and `offline-files-decisions.md`,
which stays as it is: many files are many creates.

Not built yet. Meant for a library at once, thousands of small images
say: up to 5,000 files in a list, each up to the 25 MB of one file. A
list of large files takes as long as it takes; splitting it is the
person's choice.

## Decisions

- **Each file is created on its own.** A file that fails leaves the
  others created; there is no all-or-nothing. The bucket cannot join a
  database transaction, and losing 49 good uploads to one bad one is
  worse than a list that says which one failed.
- **No new events or tables, and one new read.** The page calls
  `filesCreate` once per file, as the single-file page does: on the web
  the checksum, the signed upload, the PUT and the create; in the app
  the bytes kept on the device and the create event queued, sent by the
  sync. The one new read finds duplicates for many checksums at once,
  see "Duplicates".
- **One page.** The New page takes one file or many; a list of one is
  what it is today.
- **Up to 5,000 files in a list.** Picking past it adds none of the
  extra and says so.
- **One choice of communities for the whole list**, the current one to
  start, as the single-file page picks. A file is put in other
  communities afterwards from its Edit page.
- **Review first.** Picking files fills a list; nothing is created until
  the person presses Create.
- **Each duplicate is decided in its row**, with a way to give the same
  answer to every row not yet decided.
- **The person stays on the page while the files go.** Nothing keeps
  going for them elsewhere.

## The list

Files come from the picker, which selects many, or are dropped on the
list. One row per file, in the order picked; picking or dropping again
adds to the list. Rows needing a decision, and after Create the failed
ones, are at the top. Each row has:

- the file's name, its own name to start, shown as text and edited
  when clicked, so 5,000 rows are not 5,000 inputs;
- its size;
- a duplicate notice and choice, when it has one;
- a button to take it off the list, before Create;
- its status, after Create.

## Duplicates

Each file's checksum is worked out once picked, as now, and kept for
the create, so the bytes are read for it once. The page asks for the
files the person sees with any of the list's checksums at once,
`filesListDuplicates`, 500 checksums a request: on the web
`POST /api/files/duplicates`, by `files::list_duplicates` and the same
predicate as every read; in the app `files_list_duplicates`, the
device's own rows. A row is a duplicate of either:

- **a file the person sees** with its checksum, the newest shown as on
  the single-file page. Choices: **Put the existing
  file in the communities chosen**, offered when it is missing from one
  of them, **Create a copy** or **Skip**;
- **an earlier row of this list** with the same bytes. Choices:
  **Create a copy** or **Skip**.

A duplicate starts undecided, and Create waits until every duplicate is
decided. Beside a row's choice, **Apply to the rest** gives that choice
to every row still undecided that offers it; a row that does not offer
it stays undecided.

## Creating

Create runs the rows in order:

- **Created and Copy rows** call `filesCreate` with the row's name, the
  communities chosen and the checksum already worked out. On the web
  three at a time, each with its own progress; in the app one at a time,
  since each is only a local write.
- **Put rows** call `filesUpdate` on the existing file with its
  communities plus those chosen, as the single-file page does.
- **Skip rows** do nothing.

A bar at the top says how many of the list are done. A row's status is
waiting, uploading (with its progress, on the web),
created, put in, skipped or failed (with the reason, the server's field
messages for a wrong size or name). The page stays on the list when it
is done and says what happened: "10 created, 1 put in, 2 skipped, 1
failed". **Retry failed** runs the failed rows again; their names stay
editable, so a refused name can be fixed first. A link goes to the
files.

In the app, "created" means kept on the device; the sync sends it, and
the files index shows each one waiting until then, as for one file.

## The sync in the app, with thousands waiting

- **One sync for the list.** `files_create` starts a sync after each
  file, and syncs wait on one lock. A sync started while another is
  already waiting to start is dropped, since the waiting one will see
  its changes; every write gains from this.
- **A sync takes at most 100 waiting files.** It asks about them, sends
  their bytes and pushes their events, then starts another sync when
  more are waiting. So a signed URL, good for 10 minutes, is used well
  within them, rather than expiring behind thousands of others and
  counting a try against its file; and posts and pulls wait for 100
  files, not 5,000.
- **To measure**: a push of 500 file creates, each checked in the
  bucket by the server before it is written, within the push's time.

## Staying on the page

While files are going, closing or reloading the tab asks first
(`beforeunload`). A link to another page inside the app is not stopped:
the requests already started finish, and their results are not shown.
Keeping a list going across pages is left for later.

## Left for later

- **A list that keeps going across pages**, a queue in `lib/` the pages
  read, and one that survives closing the tab, which needs resumable
  uploads.
- **Batch creates**: one request signing many uploads and one
  creating many files. They save round trips (three per file on the
  web), and are worth it only if a long list feels slow. Signed URLs
  last 10 minutes, so they would still be signed a few at a time as
  the uploads go.
- **Uploads in a task of their own** in the app (`docs/suggestions/files.md`).
  The sync holds its lock while the bytes go, so a long list holds back
  posts and pulls until it is sent.
- **Choosing communities per file** in the list.
- **Picking a folder**, whose structure files have nowhere to keep.

## Other resources

Files are dominated by their bytes, so each is its own create. Rows
without bytes, posts from a spreadsheet say, are the other case: one
database, so all-or-nothing in one transaction is natural and usually
wanted, with every row checked before any is written.
