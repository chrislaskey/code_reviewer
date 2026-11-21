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
- **Links to the app** are plain paths, `/accounts/sign-up`: a full page
  load, answered by the app's `index.html`.

- **Signed in**, the header shows "Open the app" in place of Sign in
  and Sign up. The session cookie is not readable by a page, so the
  app's pages keep a hint in localStorage, `app:signed-in`
  (`svelte/src/lib/current_user.svelte.js`), and a script in the header reads
  it before the links paint. A stale hint costs a click: `/home` shows
  Sign in.

To add a page, add its file. The web server finds it when it starts.

## Which paths are the public website's

The app owns the top level and is the fallback: any path nothing else
answers gets the app's `index.html`. The public website takes only:

| Path | Answer |
|---|---|
| `/` | `public/index.html` |
| `/<name>` | `public/<name>.html`, for each file the build has |

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
