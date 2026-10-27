# Test call stack: follow-ups

The spike at `/review/test-call-stack` lists the tests of the head and walks the
calls below one test top to bottom (`core/src/analysis/test_call_stack.rs`).
Things it does not do yet, in rough order of value.

## Calls injected by `use` are invisible

`live/2`, `render/1`, `element/2`, `assign/3` and friends arrive through
`use ShopWeb.ConnCase` or `use ShopWeb, :live_view`, not through a literal
`import`. The index drops local calls it cannot resolve, so these never show
as system calls. Options: record unresolved local calls as "unknown" frames;
or follow `use` into the `__using__` macro and collect its `import`s.

## Find the LiveView the test drives

A LiveView test rarely calls the LiveView module directly. `live(conn, "/orders")`
goes through the router. Resolving the path against `router.ex` (`live "/orders",
OrderLive`) would let the walk continue into `mount/3`, `handle_params/3` and
`render/1` of that LiveView, which is the stack a reviewer actually wants.
Likewise `render_click(view, "refresh")` could continue into `handle_event/3`.

## Better LiveView test detection

`is_live_view_test` is a guess from the path (`/live/`) and module name
(`LiveTest`). Reading the `use` line of the test module, or the case template
it uses, would be exact.

## Tests that reach a change

The picker marks tests whose own body changed. Marking tests whose stack
touches a changed function needs a walk per test; it could run for the shown
tests only, or be precomputed from the incoming call trace of each change.

## Walking aids

- Keyboard: `j`/`k` or arrows to move between frames, `n`/`p` between changes.
- Collapse a whole subtree under a frame.
- Fold runs of unchanged frames, like diff context.
- Show the calling line inline, so the walk reads as "line 11 calls ...".

## Working tree and large repos

The walk reads whole files from git once per file, so a stack through hundreds
of files is slow on a commit comparison. A text cache per index would fix it.

## ReactFlow version

`/review/test-call-stack/flow` draws the same frames on a canvas, with three
weights: changed functions as cards with their diff, unchanged project
functions as one-line cards, calls outside the project as dashed pills. Node
heights are computed from the diff line count (`flow_nodes.tsx`), so the
layout never measures the DOM. Ideas:

- Hover a frame to highlight its callers and calls, as the chips view does.
- Collapse a whole subtree under a frame, hiding its edges too.
- Drop the dedicated node type once the ledger node can show a code footer.
