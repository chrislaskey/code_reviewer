# Code Reviewer desktop app

A [Tauri](https://tauri.app) app: Rust commands in `src-tauri/`, pages in
`src/` as a [Svelte](https://svelte.dev) app built with [Vite](https://vite.dev),
one HTML entry per page. What it does and why it is built this way is in
`../README.md` and `../docs/decisions.md`.

## Running

Prerequisites: Rust (stable), the Tauri CLI
(`cargo install tauri-cli --version '^2' --locked`), Xcode command line
tools, and Node and pnpm (see `.tool-versions`). Install the packages once
with `pnpm install`.

```sh
cargo tauri dev        # or ../bin/dev
```

`cargo tauri dev` starts the Vite dev server on port 1420 (`pnpm dev`) and
loads the window from it, so changes to `src/` show up without a restart.
`pnpm check` type-checks the Svelte components, `pnpm build` builds `dist/`,
and `cargo tauri build` makes the macOS app bundle under
`../target/release/bundle`.

Logs go to stderr. Set `RUST_LOG=code_reviewer_lib=debug` for more.

## Pages

One HTML entry, `src/index.html`, and a router (`src/lib/router.js`,
`sv-router`) that shows the page at the path, swapped in place without a
page load:

| Path | Shows | Does |
| --- | --- | --- |
| `/` | `HomePage.svelte` | Links to the three features |
| `/reviews/new` | `reviews/ReviewNewPage.svelte` | The form: repository, what to compare, rules, groups |
| `/reviews/show?repo_path=…` | `reviews/ReviewShowPage.svelte` | Runs the comparison in the URL and shows the Node and Ledger views |
| `/traces/new` | `traces/TraceNewPage.svelte` | The form: repository, what to compare |
| `/traces/show?repo_path=…` | `traces/TraceShowPage.svelte` | Lists the tests of the comparison and walks the calls below the picked one |
| `/sessions` | `sessions/SessionsIndexPage.svelte` | The tmux sessions the app started, with the Claude Code and git status in each, then its worktrees without a session; the Review menu opens one's changes in Reviews and the Complete menu finishes one. Refreshed every 5 seconds |
| `/sessions/new` | `sessions/SessionNewPage.svelte` | Creates a worktree of a repository and a session in it, both named `cr-<name>`, and opens it in the terminal app |

Pages link to each other by path with plain `<a href>` links, which the
router intercepts, and only through `src/lib/routes.js`, which knows every
page's path and query parameters; code that navigates calls `navigateToUrl`
in `src/lib/router.js`. Every path is reloadable: the dev server and the
bundled app both answer any path with `index.html`. A page's input is its
query, flat and named after the core crate's request fields
(`/reviews/show?repo_path=…&base_ref=main&head_ref=cr-fix&compare_from_merge_base=true`,
a list as a repeated key), so a review is a link, from the form, from a
session card, or from a bookmark.

## How the pages talk to Rust

Every call goes through `src/lib/backend.js`, one function per Tauri
command, with the request and response shapes of the `core` crate. Nothing
else imports `@tauri-apps/api`. The commands are in `src-tauri/src/lib.rs`
(sessions) and `src-tauri/src/review.rs` (reviews), listed in
`src-tauri/build.rs` so each gets an `allow-*` permission, which
`src-tauri/capabilities/default.json` grants to the main window.

## Styling

`src/app.css` runs Tailwind v4 with daisyUI and its built-in light and
dark themes (`src/lib/theme.css`), and makes the `hero-*` icon classes from
the `heroicons` npm package (`vendor/heroicons.js`). The theme also sets the
font, Plus Jakarta Sans, vendored in `src/public/fonts` (copied from the
Starter Pack, OFL licence alongside) and served from `/fonts`, so it works
offline. The app's own styles
(`src/styles.css`, and `src/reviews/styles.css`, `src/sessions/styles.css`
for each feature) are in Tailwind's base layer, so utility classes in markup
win over them. Don't give the app's own elements class names daisyUI also
defines, like `card`, `status`, `label` or `container`.

Each page loads its stylesheet with a `<link>` in its HTML `<head>`, not an
`import` in its JS entry. In dev, Vite only injects imported CSS once the
module runs, so the page first paints unstyled and flashes white on every
navigation.

The theme is chosen by `data-theme` on `<html>`. `src/lib/theme_script.js`
sets it before the first paint and switches it on the `phx:set-theme` events
the theme menu dispatches, keeping the choice in `localStorage`;
`vite.config.js` inlines the script where a page has the
`<!-- theme-script -->` marker.

## Layout

```
src/
  index.html, main.js                    the one document, mounting the router
  HomePage.svelte                        the home page
  app.css, styles.css                    Tailwind with the theme, and the app's and every page's styles
  public/fonts/                          Plus Jakarta Sans, served from /fonts
  lib/
    backend.js                           every call to the Rust side
    routes.js                            every page's path and query parameters
    router.js                            the router: the page at each path, and navigateToUrl
    components/                          Button, Header, Icon, ThemeMenu, ChoiceMenu, MenuChoice, the AppNav gutter and
                                         RemountPageOnNavigation around every page
    theme.css, theme_script.js           daisyUI themes and the script that applies them
    log.svelte.js, LogList.svelte        the log the sessions pages show
  reviews/
    ReviewNewPage.svelte, ReviewShowPage.svelte
    styles.css                           the review pages' styles
    review_form.js                       the form, its storage, the request it builds and back
    view_model.js                        the backend's files and group rows as the views draw them
    change_styles.js                     colours and markers: what changed, what is risky
    nodes/                               the Node view: SvelteFlow canvas, group card and link chip nodes, layout
    ledger/                              the Ledger view: ledgers, lines, code footers, syntax colours
  sessions/
    SessionsIndexPage.svelte, SessionNewPage.svelte
    styles.css                           the sessions pages' styles
    TmuxSession.svelte, GitWorktree.svelte, ReviewMenu.svelte, CompleteMenu.svelte, GitStatus.svelte, ClaudeCodePill.svelte, ...
vite.config.js                           Tailwind, the theme script and the dev server port
vendor/heroicons.js                      the Tailwind plugin making the hero-* icon classes
src-tauri/
  src/lib.rs                             the app: state, the sessions commands
  src/review.rs                          the review commands around the core crate
  src/sessions.rs                        detects the settings, names sessions with the prefix, lists,
                                         creates and shows them
  src/tmux.rs, git_worktree.rs, git.rs, terminal_app.rs, process.rs, claude_code.rs
                                         runs tmux, adds, lists, merges and closes git worktrees, reads a
                                         directory's git status, controls terminal apps, lists processes
                                         and reads Claude Code's session statuses
  Info.plist                             merged into the app bundle's Info.plist
  capabilities/                          which window may call which command
  tauri.conf.json                        the Tauri configuration
```
