# A file's content on its row

A file's content is in the bucket, or on its row. Every file works the
same either way: it has a name, it is in communities, it syncs and is
deleted the same way. Only where its content is kept differs. A row
keeps small content, such as an emoji or a short SVG, so nothing is
uploaded for it.

`offline-files-decisions.md` holds how bytes in the bucket are kept,
sent and fetched; `files-communities.md` holds who a file belongs to.

## Decisions

- **`content` on `app_files`, nullable.** Set, the content is on the
  row and nothing is in the bucket. Empty, the bytes are in the bucket
  at `files/<created_by_id>/<file id>`, as before.
- **No column says which.** The bucket's key is worked out from the
  row, so `content` alone tells them apart. A third place, such as a
  link to another site, would add its own column then.
- **Content is text.** Binary content goes in the bucket.
- **`content_type` says what the content is**, as for an upload:
  `text/plain; charset=utf-8` for an emoji, `image/svg+xml` for SVG.
- **`size` and `sha256` are the content's**, worked out when the file
  is checked, so the columns stay required and a duplicate is found the
  same way.
- **At most 64 KB** (`MAX_CONTENT_BYTES`). Every member's device gets
  the row, so content on it stays small.
- **The content never changes**, as bytes never do. An update changes
  the name only.

## How it travels

- **The create** carries `content`. The server has nothing to look for
  in the bucket, and a device has nothing to send: `ask_about_waiting`
  skips the create, so it is pushed with the rest.
- **The pull** brings the row, its content with it.
- **A page on a device** asks for the file's local copy, as for any
  file. `bytes::local_copy` writes the content to the account's folder
  the first time, with no fetch, working offline too. The cleanup keeps
  it while the row is live, as any copy.
- **A page on the web** asks for the file's URL. `bytes::download_url`
  answers a `data:` URL, in base64. It has no `Content-Disposition`, so
  saving one under its name is the page's.

`SyncedBytes` has two methods for this, with defaults, so a table whose
rows never keep content changes nothing: `content_of_create`, the
create's `content` field, and `content`, which answers none.

## Shown safely

A page shows a file by its content type, `ShownAs`, whatever its
content's place. SVG is an image: shown in an `<img>`, its scripts never
run. A page never puts content into its own markup.
