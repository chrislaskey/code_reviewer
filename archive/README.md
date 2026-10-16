# CodeReviewer

Hierarchical, deterministic summaries of code changes.

GitHub reviews a change one file at a time. `CodeReviewer` reads the same
change top-down instead: which modules changed, which public and private
functions inside them, which tests, and only then which lines. Outlines
come from AST when a file parses and from a line-oriented regex pass when it
does not (diff fragments, broken syntax). No LLMs, no network, same output
every run.

## Usage

```sh
# one commit in another repository
mix code_reviewer.review --repo ~/code/form_flow --rev b67155b

# any two revisions, written to a file
mix code_reviewer.review --repo ~/code/form_flow --from v0.3.0 --to main --out review.txt

# a diff on stdin (best effort: only lines present in the diff are known)
git -C ~/code/form_flow diff HEAD~3 | mix code_reviewer.review --stdin

# check the regex parser against the AST parser on any codebase
mix code_reviewer.compare_parsers ~/code/form_flow/lib ~/code/form_flow/test
```

## How it works

```
git diff  ──►  CodeReviewer.Diff        unified diff -> files, hunks, numbered lines
                     │
full file text ──►  CodeReviewer.Outline  modules, functions (name/arity/visibility/range),
(from git, both      ├─ Outline.AST        directives, @doc/@spec/@impl, ExUnit tests
 sides)              └─ Outline.Regex      same shape, works on fragments
                     │
               CodeReviewer.Analysis     match before/after by {module, name, arity},
                     │                   attribute hunks to functions, assign verbs
               CodeReviewer.Review       plain structs (see ideas-v09.md)
                     │
               CodeReviewer.Report.Text  flat text; other renderers plug in here
```

Verbs at every level: `created`, `deleted`, `updated`, `renamed`,
`unchanged`. Updated functions carry details: `body`, `head`, `clauses`,
`visibility`, `spec`, `doc`, `impl`.

When the repository is available, both sides of every file are read from
git and the AST parser is used. From a bare diff, the regex parser runs on
the visible fragment and the module name is taken from the `@@` hunk header.
Files processed that way are marked `(fragment)` in the output.
