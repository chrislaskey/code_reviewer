## Development

Two mix projects share one repository and one `CodeReviewer` namespace:

- `/` (this directory) is the core library: diff parsing, outlines,
  analysis, the text report, and the `mix code_reviewer.*` tasks.
- `/code_reviewer` is a Phoenix app that is the UI for the library. Its
  `mix.exs` adds the root `lib/` to `elixirc_paths`, so the library compiles
  into the web app directly. There is no path dependency because both
  projects are named `:code_reviewer`.

### Core library

```sh
mix test
mix code_reviewer.review --repo ~/code/form_flow --rev b67155b
mix code_reviewer.compare_parsers path/to/some/elixir/repo
```

### Web UI

```sh
cd code_reviewer
mix setup            # first time
mix phx.server       # http://localhost:4000
mix precommit        # compile --warnings-as-errors, format, test
```

Open `http://localhost:4000/?repo=~/code/form_flow&rev=b67155b` to load a
specific commit. With no params the page reviews `HEAD` of this repository.

There are two views of the same review, linked from the header, and both
read `repo` and `rev` from the query string (`CodeReviewerWeb.ReviewSource`):

- `/` is the function table described below.
- `/files` is the file view, laid out like a GitHub pull request's "Files
  changed" tab: a collapsible file tree on the left, one card per file on
  the right with its hunks as a unified diff. Cards fold from the chevron in
  their header; the "Viewed" checkbox folds a card and counts toward the
  progress bar in the summary. Everything here is server side and
  per-file (the cards are a LiveView stream), so folding one file
  re-renders only that file.

The file view has its own vim-style navigation (`assets/js/hooks/vim_files.js`)
with two panes: the tree on the left and the diff on the right.

| Keys | Action |
|---|---|
| `j` `k`, arrows | in the tree: previous / next file; in the diff: previous / next line, flowing from one file into the next |
| `Enter` | in the tree: scroll the diff to the highlighted file; on a diff line: report the line to the server (`activate`) |
| `l` / `→` | move into the diff of the highlighted file (expanding it if it is folded) |
| `h` / `←` / `Escape` | back to the tree, on the file the cursor was in |
| `n` / `N` | next / previous change (a run of `+`/`−` lines) in the diff |
| `{` / `}` | first line of the previous / next file in the diff |
| `gg` / `G`, `5j` | first / last, counts as a prefix |
| `Ctrl-d` / `Ctrl-u` | half page down / up |
| `za` / `zo` / `zc` | toggle / open / close the highlighted file's diff |
| `zR` / `zM` | expand / collapse every file |

The function table has vim-style navigation, implemented client side in
`assets/js/hooks/vim_grid.js`:

| Keys | Action |
|---|---|
| `h` `j` `k` `l`, arrows | move the cursor one cell |
| `5j`, `3l` | counts work as a prefix |
| `gg` / `G`, `3G` | first / last row, row 3 |
| `0` `^` / `$` | first / last column |
| `Ctrl-d` / `Ctrl-u` | half page down / up |
| `{` / `}` | previous / next table row, skipping diff lines |
| `n` / `N` | next / previous change (a run of `+`/`−` lines) in a diff |
| `Enter` | on a module row: expand or collapse its functions; on a module row's `+/−` cell: show or hide a whole-module diff; on a function row: show or hide that function's diff; on a diff line: report the line to the server (`activate`) |
| `za` / `zo` / `zc` | toggle / open / close the fold under the cursor: the diff on a function row or diff line, the module on a module row. `zc` on a function whose diff is closed closes its module, as in vim |
| `zR` / `zM` | open / close every module |
| `Escape` | clear pending keys; on a diff line, jump back to the row that owns the diff |

The table is a module view by default: one row per module with its functions
folded underneath. Folds, like movement, are client side. Diffs are lazy: the
first `Enter` on a function fetches both versions of its file from git,
builds a whole-function line diff (`CodeReviewer.FunctionDiff`), and streams
one row in under the function. An open diff behaves like an open fold: its
lines are cursor stops, so `j` from the function walks into the code and out
the other side, and the status line switches from `col` to `line` with the
line number under the cursor. Folding the module hides its functions' diffs.
