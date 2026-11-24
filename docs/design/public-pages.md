# Public pages

The public website: what a visitor sees before signing up. It is the
first of the web server's apps beside the application itself, and the
smallest: it has no tables, no API and one line of JavaScript.

## Plain HTML, one file a page

A page is an HTML file in `svelte/src/public/`: `index.html` is the home
page, `about.html` is `/about`. A search engine and a link preview read
the file as it is; nothing is drawn by a script.

- **The build** (`svelte/vite.config.js`) takes every `.html` file in the
  folder as an entry and writes it to `dist/public/`, with the same
  stylesheet as the app's pages (`app.css`: Tailwind, daisyUI, the
  themes, the font) and the theme script.
- **The header and footer** are written once, in
  `svelte/src/public/partials/`, and the build puts them where a page has
  `<!-- public-header -->` and `<!-- public-footer -->`.
- **Links to the app** are plain paths, `/accounts/sign-in`: a full page
  load, answered by the app's `index.html`.

- **Signed in**, the header shows "Open the app" in place of Sign in
  and Sign up. The session cookie is not readable by a page, so the
  app's pages keep a hint in localStorage, `app:signed-in`
  (`svelte/src/lib/current_user.svelte.js`), and a script in the header reads
  it before the links paint. A stale hint costs a click: `/home` shows
  Sign in.

To add a page, add its file, its English marked `<t>…</t>` (below). The
web server finds it when it starts.

## In other languages

Each page is in every language the app is, `/es/about` for `es`, with
its own catalog, `svelte/translations/public/`, apart from the app's
pages' `svelte/translations/`, so neither writes over the other.

- **A page marks its English**: `<t>…</t>` around each paragraph, heading
  and list item, with the links and bold text inside it, so a sentence
  stays one message; around a link's or a button's text; in the
  `<title>` and the description's `content`. The marks never reach a
  browser. Text that stays as it is, the app's name, has
  `<!-- untranslated: why -->` before its element.
- **The messages** are that English, its whitespace one space,
  extracted to `translations/public/default.pot` by
  `pnpm translations:extract`, merged into each `<locale>.po` by
  `translations:update`, and checked by `translations:check` with the
  app's: a page's English outside `<t>`, a stale `.pot`, a message
  untranslated or fuzzy, all fail `bin/lint`. The links' HTML and the
  `[placeholders]` stay as the English has them.
- **The build** (`svelte/vite.config.js`, with
  `svelte/scripts/public_pages_translations.js`) writes each page in
  English at its place, and in each language to `dist/public/<locale>/`:
  `<html lang>` set, its links to public pages in the same language
  (`/es/terms`), and the footer's `<!-- public-languages -->` the page in
  every other language, each in its own name. The dev server translates
  a page as it answers it.
- **The web server** answers `/<locale>` and `/<locale>/<name>` for each
  language of `app::locales::SUPPORTED`, a core test keeping both
  catalogs to that list.
- **The app's links** to the terms and the privacy policy open them in
  the language the app shows.

Nothing sends a visitor to their language on its own: `/` is English,
and the footer links each page's other languages. A translated legal
page is a translation for a lawyer to review, as the English is a
template.

## Which paths are the public website's

The app owns the top level and is the fallback: any path nothing else
answers gets the app's `index.html`. The public website takes only:

| Path | Answer |
|---|---|
| `/` | `public/index.html` |
| `/<name>` | `public/<name>.html`, for each file the build has |
| `/<locale>`, `/<locale>/<name>` | `public/<locale>/…`, the page in that language |

The app's own home page is `/home`, on the web and in the apps alike,
so `/` is never two pages. A public page must not be named like a path
of the app's (`home`, `posts`, `accounts`).

- **The web server** does this in `axum/src/public_pages.rs`.
- **The dev server** does the same in `vite.config.js`, since in dev the
  browser loads its pages from it.
- **The desktop and phone apps** never show a public page: their window
  opens at `/home` (`tauri/tauri.conf.json`), and the app's router sends
  `/` there too. The files are in their bundle, unlinked. The terms
  and the privacy policy, linked from the sign-up and account
  settings pages, open on the web server in the system browser
  (`tauri/src/public_pages.rs`).
