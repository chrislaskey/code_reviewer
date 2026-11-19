# Files: what was left out

Built from `docs/design/offline-files-decisions.md`: a `files` resource
whose rows sync like posts, whose bytes are kept on the device that
added them and sent to the bucket by the sync, fetched by another
device the first time it shows the file, with a limit of tries, refused
files kept for the person, and the cleanups on both sides. Left for
later, on purpose:

- **Replacing a file's content.** The bytes are fixed today; replacing
  them needs a key per content and a stored key that merges like a
  field.
- **Uploads in a task of their own.** `files::upload_waiting` runs
  inside the sync and holds its lock while bytes go. An app with large
  files calls it from its own task, then starts a sync; no data changes.
- **Multipart and resumable uploads.** One PUT per file, started over
  when cut off.
- **Uploads while the app is in the background.** A phone suspends the
  app; the upload starts over when it comes back.
- **Audio and video.** WebKit asks for a media file in ranges, which
  the `files` protocol would have to answer.
- **More types shown in place.** `files::ShownAs` knows images, plain
  text and PDF.
- **Download on phones**, through the system's share sheet, which
  needs a plugin.
- **Copying an image on Windows, Linux and Android.** macOS, iOS and
  the web are built. On Windows and Linux,
  `tauri-plugin-clipboard-manager`'s `write_image`, a still, or the
  platform's own formats for an animation. On Android, a `content://`
  URI from a FileProvider, in a small Kotlin plugin.
- **Thumbnails.** The index shows each image whole, scaled down by the
  page, so it fetches every image's full bytes.
- **Bytes stored once for many files.** Files with the same checksum
  each keep their own bytes today. To store them once, safely:
  - Every client uploads its bytes, under its own prefix as today. The
    server never skips an upload because a client names a checksum it
    already has: that would answer anyone whether a file exists in the
    deployment, and give a row to a client that never held the bytes.
  - Once the bucket has checked the bytes, the server moves them to
    `blobs/<sha256>` when no blob is there and deletes the copy, then
    the row names the blob. This saves storage, not the upload.
  - The key is the row's checksum, not its creator and id, and the
    sweep deletes a blob no live row names.
- **A file attached to a post.**
