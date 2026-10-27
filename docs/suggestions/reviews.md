# Reviews: left out of the MVP

Ideas and prototype features not in the rewrite. None are planned.

## From the prototypes

- The file-level views on the canvas: chips and blocks around a file card,
  with one edge per caller and call (`existing/frontend/src/explorations/`).
- "Open all" and "Close all" for every ledger and code block.
- The "Dimmed / Hidden" choice for calls into dependencies and the standard
  library. They are always folded into a count now.
- The "Ledger" code layout with one closed block per function, next to the
  one-block layout the MVP has.
- The test call stack: list the tests of the head and walk the calls below
  one test, as a page and on a canvas (`core` still has
  `list_tests` and `get_test_call_stack`).
- Call traces above or below a function (`core` still has `get_call_trace`).
- Highlighting the tree item of the section on screen as the page scrolls.

## New

- Keep the view (Nodes or Ledger) and the open group in the show page's URL,
  so going back and forward keeps the place.
- Open links in rendered Markdown in the browser. Tauri's opener plugin
  would do it; clicking a link does nothing now.
- Remember recent repositories on the new review page.
