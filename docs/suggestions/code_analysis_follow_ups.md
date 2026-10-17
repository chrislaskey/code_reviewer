# Suggestions: Code Analysis Follow-Ups

Ideas that came up while building `core/src/analysis/` and were left out on purpose.

Done since the first version: wiring into Axum, Tauri and `BackendAdapter`,
reviewing a commit or revision range, and added/removed/renamed functions.
See `docs/plans/code_analysis_parsing.md`.

- **Compiler-backed accuracy mode.** An optional step that runs the Elixir
  compiler tracer and replaces name/arity guesses with real call data when
  the project compiles.
- **`use` and behaviour awareness.** Resolve imports injected by `use`, and
  treat protocol and behaviour callbacks as edges to their implementations.
- **On-disk index and file watcher.** Not needed at current timings
  (about 100-150 ms cold for 3,000 functions). Revisit for repos 10x larger.
- **More languages.** Add a tree-sitter grammar crate and one
  `LanguageAnalyzer` per language.
- **Interleaved clauses.** Clauses of one function that are separated by
  other code show that code in the source view. Elixir warns about it, so it
  is rare.
- **UI pages.** The adapter methods exist, but no page uses them yet.
- **Cheaper commit indexes.** Indexing a commit reads every `.ex`/`.exs` file
  through `git cat-file --batch` (about 1.5 s for 360 files, once per commit,
  then cached). Only the files that differ from an already indexed commit
  need re-parsing.
- **Rename matching beyond identical bodies.** Renames are found only when
  the body is unchanged. A rename plus an edit shows as removed + added.
  Similarity scoring would catch it.
- **Cache size limit.** The cache keeps the newest 8 commit indexes and one
  working tree index per repository, with no memory limit.
