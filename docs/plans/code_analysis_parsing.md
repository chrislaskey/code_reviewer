# Code Analysis: Parsing Strategy

## Goals

Two generic building blocks, Elixir only for now:

1. **Full function source.** Show the whole function that contains a change,
   not just +3/-3 lines of diff context.
2. **External calls.** For each changed function, the calls coming IN from
   other modules and going OUT to other modules.
   - Level 1: one level deep, computed for every changed function.
   - Level 2: full-depth call tree, computed on demand for one function.

Both need the same thing underneath: a map of every function in the project
and who calls whom.

## What an AST parser gives us

Source text is a flat string. A parser turns it into a tree of nodes
(`call`, `alias`, `do_block`, ...), each with a type and start/end positions.
That gives exact function boundaries and distinguishes `Foo.bar()` in code
from `Foo.bar()` in a string or comment, which regex cannot.

Parsing alone knows nothing about meaning. It does not know what `Foo`
aliases to, or which function a call lands on. We add that on top
(alias resolution, name/arity matching). That layer is language specific in
its details but shared in shape.

Two kinds of parser exist:

- **Compiler-backed** (Elixir's `Code.string_to_quoted/2`, Elixir's own
  compiler tracer). Exact, and can see macro expansion. Needs the BEAM
  runtime, and strict parsers reject incomplete or broken code.
- **Tree-sitter** (grammar-driven, in-process). Needs no compiler, tolerates
  syntax errors, and has a grammar for almost every language. It sees source
  as written, so macro-generated code is invisible.

## Options considered

| Option | What it is | Fit |
| --- | --- | --- |
| Elixir `Code.string_to_quoted` (what `archive/` uses) | Elixir's own parser, run in the BEAM | Exact for Elixir. Needs Elixir installed on every machine, a subprocess or NIF bridge from Rust, rejects broken files. Does not transfer to other languages. |
| [`dexter`](https://github.com/remoteoss/dexter) | Go-based Elixir LSP. Tree-sitter plus a SQLite index of definitions and references | Built for editor go-to-definition, not for call graphs. A separate Go binary and daemon to ship and talk to, Elixir only, and its index schema is not ours. Worth reading for ideas. Not worth depending on. |
| **tree-sitter from Rust** (`tree-sitter` + `tree-sitter-elixir` crates) | Parser library linked into our Rust core | In-process, no runtime dependency, error tolerant, same API for every language. Chosen. |
| Rust-native Elixir parser | No maintained crate exists | Would mean writing our own. Rejected. |
| Compiler tracers (`mix compile`) | Exact call data from Elixir compile events | Most accurate, but needs a compiling project with deps fetched. Poor fit for reviewing a branch that may not compile. Possible later as an optional accuracy upgrade. |

## Decision: one Rust core with per-language tree-sitter grammars

- Parsing lives in Rust, behind a `LanguageAnalyzer` trait. A new language is
  one grammar crate plus one analyzer file that turns the tree into
  language-neutral `FileSymbols` (functions + raw calls).
- Everything after that is shared: the project index, call resolution,
  external-call lists, call traces and function source extraction.
- This keeps the frontend thin, matches the rest of the architecture (core
  logic in Rust, shared by Axum and Tauri), and avoids a runtime per language.

Per-language standalone tools (like `dexter`) would mean one process,
protocol and data model per language. Tree-sitter gives one of each.

## Keep it warm or parse every time?

Measured on real local Elixir projects (release build, 8+ cores):

| Project | Files | Functions | Cold index | Re-check, nothing changed | Avg full trace | Worst full trace |
| --- | --- | --- | --- | --- | --- | --- |
| app-platform | 417 | 2,644 | 108 ms | 30 ms | 5 µs | 164 µs |
| form_flow | 361 | 3,421 | 154 ms | 26 ms | 419 µs | 3.2 ms (hit the 5,000 node cap) |

Findings:

- Parsing is cheap. One file takes about 1 ms.
- Incoming calls cannot be answered from the changed files alone. You need
  every file's calls, so even Level 1 needs a whole-project index.
- Once the index exists, a full-depth trace is a graph walk over memory and
  costs microseconds to a few milliseconds. It never needs to re-parse.

So the answer is **parse once, keep the index, never parse per trace
request**:

- `ProjectIndex` is immutable and shared behind an `Arc`.
- `ProjectIndexCache` keeps one per repository. Each lookup lists files
  (`git ls-files`), compares modified time and size, and re-parses only the
  files that changed. If nothing changed, the same `Arc` comes back.
- A cold start costs about 100-150 ms, so this is a convenience more than a
  necessity. It keeps interactive requests near 25 ms (mostly the `git
  ls-files` call) and leaves room for much larger repos.
- Each trace has a node cap (default 5,000) because the tree form of a call
  graph can explode on highly connected code.

Not done yet: an on-disk cache and a file watcher. At these timings there is
no need. See `docs/suggestions/`.

## How calls are matched

Source is parsed as text, never compiled, so matching is by name and arity:

- `Mod.fun(a)` resolves `Mod` through `alias` (including `alias A.{B, C}`,
  `as:`, nested `defmodule`, `__MODULE__`). If the project defines it, the
  edge points at that definition.
- `fun(a)` is a local call if the same module defines `fun/arity`, otherwise
  an imported call if an `import` (with `only:`) provides it. Anything else
  (`if`, `Kernel` functions) is dropped.
- `x |> fun(a)` counts as arity 2. `&Mod.fun/2` counts as arity 2.
  `def f(a, b \\ 1)` accepts arities 1 and 2.
- `defdelegate` is a function with a call to its target.
- ExUnit `test "..."` blocks are zero-arity functions, so tests show up as
  callers.
- Calls to modules the project does not define (`Enum`, deps) are kept and
  marked `defined_in_project: false`. They are never expanded.

"External" means the other function is in a different module.

### Known blind spots

- Macro-generated functions and `use`-injected imports (for example
  functions from `use Phoenix.Component`).
- Dynamic dispatch: `apply/3`, `module.fun()` with a variable module,
  behaviours and protocols (calls go to the protocol, not to each impl).
- Zero-arity calls written without parentheses (`foo`), which are
  indistinguishable from variables without scope analysis.
- `import ... except:`.
- Erlang module calls (`:lists.reverse`).
- Renames with edits: a function is called renamed only when its body is
  unchanged. A rename plus an edit shows as one removed and one added.

## What was built

All in `core/src/analysis/`:

| File | Responsibility |
| --- | --- |
| `language.rs` | `LanguageAnalyzer` trait and extension lookup |
| `elixir.rs` | Elixir tree-sitter extraction (the only language-specific file) |
| `model.rs` | `FunctionId`, `FunctionDefinition`, raw call types |
| `project_index.rs` | `ProjectIndex`, call resolution, `ProjectIndexCache` |
| `function_source.rs` | Whole function with leading `@doc`/`@spec`/`@impl` |
| `changed_functions.rs` | Changed line ranges to the functions they touch |
| `external_calls.rs` | Level 1: calls into and out of a function across modules |
| `call_trace.rs` | Level 2: full-depth call tree, in either direction |
| `function_changes.rs` | Added / modified / removed / renamed functions between two snapshots |
| `service.rs` | `AnalysisService`: the one entry point the apps call |

In `core/src/git.rs`: file listing, `list_changed_line_ranges` (new-side
ranges from `git diff -U0 -M`, between two commits or against the working
tree), and commit helpers (`get_commit_sha`, `list_file_texts_at_commit`,
which reads many files with one `git cat-file --batch`).

## Reviewing a commit or a range

An index remembers where it came from (`SourceOrigin`): the working tree, a
commit, or in-memory text. Function source is read from that same snapshot,
so a removed function's source is readable at the base revision.

`AnalyzeChangesRequest` picks the two snapshots:

| `base_ref` | `head_ref` | Compares |
| --- | --- | --- |
| none | none | working tree against `HEAD` |
| set | none | working tree against `base_ref` |
| none | set | that commit against its parent (one commit) |
| set | set | `head_ref` against `base_ref` (a range) |

Refs are resolved to full SHAs first and anything starting with `-` is
rejected, so a ref can never be read as a git option. A commit never
changes, so its index is cached by SHA (newest 8 kept). Indexing a commit
the first time takes about 1.5 s for 360 files (form_flow), then it is free.

## Added, modified, removed and renamed

Both snapshots are indexed and compared function by function:

- **Added**: in the head, not in the base.
- **Removed**: in the base, not in the head. Its external callers come from
  the base index. `stale_callers` lists head callers that still call the old
  name (calls to a function that no longer exists).
- **Renamed**: a removed and an added function with the same body once the
  function name is blanked out. Only unambiguous one-to-one matches count, so
  two identical one-liners are not guessed at. This also catches a function
  moved to another module. `previous_calls_in` lists callers of the old name
  in the base.
- **Modified**: in both, touched by the diff, and the text differs. A file
  moved without edits changes no functions, even when git sees it as a
  deleted file plus an untracked one.

Each change carries its Level 1 external calls (from the head, or from the
base when removed).

## App wiring

One `AnalysisService` (it owns the index cache) lives in each app's state.
Analysis is blocking file and git work, so Axum and Tauri run it with
`spawn_blocking`.

| Core method | Axum route | Tauri command | `BackendAdapter` |
| --- | --- | --- | --- |
| `analyze_changes` | `POST /api/analyze_changes` | `analyze_changes` | `analyzeChanges` |
| `get_function_source` | `POST /api/get_function_source` | `get_function_source` | `getFunctionSource` |
| `get_call_trace` | `POST /api/get_call_trace` | `get_call_trace` | `getCallTrace` |

Axum answers 400 for a bad revision and 404 for an unknown function. No UI
page uses these yet.

Typical flow:

```
analyze_changes(repo, base_ref, head_ref)   // every changed function + Level 1 calls
get_function_source(repo, revision, fn)     // full source for display
get_call_trace(repo, revision, fn, ...)     // Level 2, on demand
```
