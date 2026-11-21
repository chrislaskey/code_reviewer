# Browsing and searching files

A person with many files, thousands of small images say, browses them
on the files index as a list or a grid and finds one by its name. The
page never holds every file: it asks for the ones it shows, a page at
a time, and the grid draws only the tiles on screen. Most people have
a few thousand files at most; a collector with tens of thousands is
served by the same design, a little slower, not by another one.

Built in `files::search`, `GET /api/files/search`, the `files_search`
command, `filesSearch` in `svelte/src/lib/core/`, and
`svelte/src/files/FilesIndexPage.svelte` with `components/FilesGrid.svelte`. The
grid is `svelte/src/lib/components/TileGrid.svelte` and the pages it asks for
`lib/paged_search.svelte.js`, which the desktop quick picker shares
(`quick-picker.md`).

## Decisions

- **One query, run where the data is.** In the app, against the
  device's SQLite; on the web, against the server's database, SQLite
  for an app starting out and Postgres once it grows. The browser never
  filters or sorts files itself, so a filter added later is a field of
  the query and a condition in its SQL, in one place, under the same
  visibility as every read.
- **A search answers the total and one page.** `FileSearch` is the
  name searched for, the content types (any when none), an offset and a
  limit; `FileSearchResults` is how
  many files match and the files at that offset, each with its state,
  creator and communities as the list had them. Only that page's
  states and communities are worked out. A limit is at most
  `files::MAX_SEARCH_LIMIT`, 500.
- **An empty name matches every file** the person sees: browsing is
  a search for "".
- **The name matches when it contains the text, in any case.** No
  wildcards: a `%` or `_` typed is looked for as itself. SQLite folds
  the case of ASCII letters only, Postgres of every letter, so "é"
  finds "É" on a Postgres server but not on a device.
- **Newest first**, by `inserted_at` then `id`. Both databases start
  from the person's communities and sort what they see; an index on
  `inserted_at` goes unused, so there is none. On a server that is the
  right plan: most rows are other people's.
- **Pages by offset.** The grid jumps to a position, the middle of the
  scrollbar say, so it asks by offset rather than "after this row".
- **Fast enough for a collector.** Measured with 50,000 files the
  person sees: a page takes 50 to 70 ms on SQLite and about 80 ms on
  Postgres, among 250,000 rows of others; the count 30 to 75 ms. A few
  thousand files take a tenth of that.
- **The person sees only their own files.** A search never spans
  other people's files beyond what the person already sees.

## The page

- **List or grid**, the choice kept in the browser
  (`files_view.js`). The list is a row each with the file's name,
  size, date, communities and state; the grid is squares of the images,
  many to a screen.
- **The grid draws only what is on screen.** Every tile is the same
  size, so which tiles are on screen is arithmetic on the scroll
  position (the page's, or the quick picker's box), the grid's width
  and the total; the grid is as tall as all
  of them and draws those on screen and a screen above and below. A
  tile scrolled away is gone, image and all, so a few hundred images at
  most are alive at once, an animated GIF plays only while seen, and
  the app fetches a file's bytes only once it is near the screen.
- **The grid asks for the pages it shows**, 200 files a page, as it is
  scrolled to them; a tile whose page has not come yet is an empty
  square.
- **The list shows the first page and loads more** with a button, a
  page at a time.
- **A search waits for a pause in typing**, then starts over from the
  top: 50 ms in the app, where it reads the device's SQLite, and 200 ms
  on the web, where each asks the server (`filesSearchWaitMs` in
  `svelte/src/lib/core/`). An answer to an older search or an older sync is
  dropped.
- **A sync starts the pages over**: rows come and go with it, and
  every position after them moves.

## Left for later

- **Filters beside the name**: a file's community, and tags once files
  have them, each a field of `FileSearch` as `content_types` is, and a
  menu on the page.
- **Other orders** than newest first.
- **A cap on the count**: past some number, "10,000+", if counting a
  collector's matches proves slow.
- **A trigram index** on Postgres (`pg_trgm`) if "contains" proves slow
  there.
- **Thumbnails**, see `docs/suggestions/files.md`.
