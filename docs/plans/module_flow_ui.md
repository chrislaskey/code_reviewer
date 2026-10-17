# Module Flow UI Plan

First product area: for each changed file, show **inputs → file → outputs**.

- **Inputs**: functions in other modules that call into the file (one level).
- **File**: the file's changed functions, with added/modified/removed/renamed.
- **Outputs**: calls from the file into other modules (one level).

One row per changed file, stacked. Full call traces come later, reached by
clicking through from a chip.

## Step 1: ReactFlow in both apps (done)

- `@xyflow/react` and `@heroicons/react` live in `frontend/` only.
- `frontend/src/theme.css` holds the shared colour tokens (`base-*` names match
  visualized-elixir) and imports ReactFlow's stylesheet. Each app's
  `styles.css` imports it.
- `frontend/src/components/card.tsx` is the visualized-elixir card pattern in
  TSX: avatar with status dot, header, stats row, tab-bar footer.
- `/explorations/reactflow-basics` renders four hardcoded files through custom
  card nodes. Checked in the browser under both the Axum and Tauri Vite configs.
  Not yet checked inside the real Tauri (WebKit) window.

## Step 2: Explorations with hardcoded data (done)

All under `frontend/src/explorations/`, routes under `/explorations`.

- `data.ts`: hardcoded example, shaped like `analyzeChanges` grouped by file.
- `breadth.ts` / `/explorations/module-flow-breadth`: nine ideas in four groups
  (shape, attribution, review signals, between files).
- `depth.ts` / `/explorations/module-flow-depth`: three refinements that combine
  the ideas that earned their place.

| Depth option | What it is | Trade-off |
| --- | --- | --- |
| E1 Review canvas | Function ports, markers, diff-aware calls, dependencies folded into one list, click a function to focus, click a linked chip to jump | Most informative. Tallest. |
| E2 Grouped | One chip per module with counts, expand on click | Fewest nodes. Hides function-level detail until opened. |
| E3 Ledger | One wide row per file, summary strip, sort by risk, collapse rows | Densest and fastest to scan. Gives up the graph. |

## Step 2b: Second round (done)

- **Theme.** `frontend/src/theme/`: `ThemeProvider` (system/light/dark, saved in
  localStorage, applied as `data-theme` on `<html>`), six switcher mockups at
  `/explorations/theme-switchers`. `TopBar` uses `SegmentedIconsSwitcher`; swap
  the `ThemeSwitcher` alias in `ThemeSwitchers.tsx` to change it.
- **Ordering.** Callers and callees are sorted by the index of the changed
  function they connect to (`orderCallers`, `orderCallees` in `data.ts`), so
  edges fan out in function order instead of crossing.
- **Top-aligned columns.** `layoutRows` starts inputs, file and outputs at the
  same top edge, so each row reads as one horizontal line.
- **Interactive depth pages.** ReactFlow was setting `pointer-events: none` on
  nodes; `theme.css` forces them back on. E1 hover previews a function's links,
  click pins it.
- **Ledger (A3, E3).** Columns are `File / Outputs / Inputs`, no edges. Hovering
  a function highlights its outputs and inputs and fades the rest.
- **Ledger variants** at `/explorations/ledger-variants`: F1 function rows,
  F2 inline pills, F3 attention list.

## Step 2c: Third round (done)

`/explorations/module-flow-variations`, built on `explorations/variants.ts`,
`ledger.tsx` and `block.tsx`. All ReactFlow canvases, so a later switcher can
move between them.

- **F1** now top-aligns the function label with its first call.
- **E3 ledger iterations (G1 to G4).** One `LedgerNode` driven by a
  `LedgerStyle`: two-line callers, attention strip, pills, test coverage.
  Columns stay File / Outputs / Inputs.
- **A2 chips (H1, H2) and A1 blocks (H3, H4).** One connection point per side.
  Hovering a function, chip or row fades everything that belongs to another
  function and thickens the related lines. H4 puts a heading per function inside
  each block.
- **Dependencies and standard library.** `OutsideMode`: dimmed (default),
  folded into one line, or hidden. Every view has the control.
- **Card slot.** `FileCardNode` takes `stats`: `counts`, `coverage` or `none`.
  Coverage is hardcoded in `data.ts` for now.

## Step 2d: Fourth round (done)

On the same variations page.

- **G1b** sorts outputs and inputs by name instead of grouping them by function.
- **H5** is H1 simplified: coverage is a dot and a percentage beside the module
  name, subtitles are file paths only (clipped at the start), all lines solid
  with dependencies thinner and lighter, changed calls get a coloured left edge
  instead of a tinted box, connectors are grey except for stale callers, and the
  Changed functions label is gone.
- **H6** is H5 with function names coloured by change kind (`CHANGE_STYLES.name`).
- Callee file paths are hardcoded in `data.ts` (`calleePath`). The backend will
  need to report the file of every called function.

## Step 3: Wired to the backend (done)

Chosen: H5 (chips), G1 (ledger) and H4 (blocks), with a toggle between them.

- **Rust.** `core/src/analysis/file_changes.rs` groups `FunctionChange`s by file
  (`group_changes_by_file`): functions with line counts, callers (stale first,
  flagged when also changed), calls out (project first), and the file status
  (all added, all removed, or modified). `AnalysisService::analyze_file_changes`
  takes the same request as `analyze_changes`.
- **Wiring.** Axum `POST /api/analyze_file_changes`, Tauri command
  `analyze_file_changes`, `BackendAdapter.analyzeFileChanges`.
- **UI.** `/review` (`pages/review/Index.tsx`): repo path and refs (remembered in
  localStorage), a View toggle (Node chips / Ledger / Node blocks) and the
  dependency control. `review/view_model.ts` maps the response onto the shapes
  the view builders already draw; `review/ReviewCanvas.tsx` picks the builder.
- **Look changes.** H5's connectors are one colour and width, solid, and do not
  thicken on hover (fading shows the focus). Chips have no coloured left edge;
  the marker icons carry the meaning. The card's dot follows the file status:
  green all added, red all removed, yellow otherwise.
- **Hidden until the backend has the data.** Test coverage, test-caller
  styling, new/removed call markers, the dependency vs standard library split,
  callee file paths and removed-line counts for modified functions. See
  `docs/suggestions/module_flow_follow_ups.md`.

## Step 4: Finalize the UI (next)

Try it on real changes, then settle the views and fill the hidden gaps.
