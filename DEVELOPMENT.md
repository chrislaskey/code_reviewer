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

The function table has vim-style navigation, implemented client side in
`assets/js/hooks/vim_grid.js`:

| Keys | Action |
|---|---|
| `h` `j` `k` `l`, arrows | move the cursor one cell |
| `5j`, `3l` | counts work as a prefix |
| `gg` / `G`, `3G` | first / last row, row 3 |
| `0` `^` / `$` | first / last column |
| `Ctrl-d` / `Ctrl-u` | half page down / up |
| `Enter` | on a module row: expand or collapse its functions; on a module row's `+/−` cell: show or hide a whole-module diff; on a function row: show or hide that function's diff |
| `za` / `zo` / `zc` | toggle / open / close the module under the cursor |
| `zR` / `zM` | open / close every module |
| `Escape` | clear pending keys |

The table is a module view by default: one row per module with its functions
folded underneath. Folds, like movement, are client side. Diffs are lazy: the
first `Enter` on a function fetches both versions of its file from git,
builds a whole-function line diff (`CodeReviewer.FunctionDiff`), and streams
one row in under the function; the cursor skips diff rows and folding the
module hides them.
