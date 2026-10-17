# Suggestions: Module Flow UI Follow-Ups

Ideas and backend gaps found while building the hardcoded explorations.

## Data the explorations assume the backend can give

- **Calls added or removed by the change.** The diff-aware options (C2, E1)
  colour calls as new or removed. `analyzeChanges` returns the head's calls
  only, so this needs a base-vs-head comparison of each function's `calls_out`.
- **Dependency vs standard library.** `CallLink.defined_in_project` only says
  inside or outside the project. The origin split (C3, E1) needs a third value:
  dependency (`deps/`) vs standard library.
- **Test vs production callers.** The explorations mark callers as tests. Needs
  a rule in core, for example the caller lives under `test/` or in an `.exs`
  file.
- **Callers that are also changed.** The link-icon jump (E1) needs each caller
  matched against the review's own changed functions.
- **Rows by file.** Changes are per function today. A file row needs them
  grouped, with caller and call lists merged per file.

- **Test coverage per file.** The cards and ledger headers show a coverage
  percentage from hardcoded data. Needs a core function that reads coverage
  output (for example lcov) and returns a percentage per file, ideally also for
  the changed lines only.

## Hidden on `/review` until the backend has the data

The mockups show these, but the real review page leaves them out:

- **Test coverage** (card dot and percentage, ledger badge).
- **Test callers.** All callers are drawn as production callers.
- **New and removed calls.** Calls out never carry the new/removed markers.
- **Dependency vs standard library.** All calls outside the project are
  dependencies.
- **Callee file paths.** Output chips have no subtitle; only the title shows.
  `FileCallee` needs the defining file.
- **Removed-line counts for modified functions.** Only added or edited lines are
  counted. Removed functions report their full length.
- **Several modules in one file.** A file shows the first changed function's
  module as its title. Calls between modules in the same file appear as inputs
  and outputs of that file.

## UI ideas not built

- **Hide boring files.** Collapse files whose only change is internal and that
  nobody outside the module calls.
- **Cluster by directory or context.** Group rows under `lib/shop/` and
  `lib/shop_web/` headers for big reviews.
- **Virtualize large reviews.** Only render rows near the viewport once a review
  has dozens of files.
- **Keyboard navigation.** Next and previous file, next stale caller.
- **Reviewed marks.** Tick a file or caller off while reviewing, remembered per
  revision range.
- **Remove the ReactFlow attribution.** Needs a ReactFlow Pro subscription.
- **Theme in the Tauri window.** Follow the OS appearance event instead of the
  media query if WebKit does not report it reliably.
- **Hover from the keyboard.** Ledger hover highlighting has no focus
  equivalent yet.
- **Keep state across views.** The view toggle exists; collapsed ledger files
  and pinned functions reset when switching.
- **Click-through to a call trace.** Chips and rows do nothing on click yet
  (except jumping to another changed file). `getCallTrace` is ready.
- **Sort and summary on `/review`.** The mock G4 sorts riskiest first and shows
  a summary strip. Files are in path order for now.
- **Coverage on changed lines.** Show coverage of just the lines this change
  touched, next to the file total.
- **Pill width without estimating.** Pill ledgers work out wrapping from
  monospace character widths. Measure for real if the font ever changes.
- **File of each called function.** Chips now show the callee's file path. The
  analysis returns modules and functions; it needs to return the file too, and
  dependencies and the standard library need a stable way to name theirs.

