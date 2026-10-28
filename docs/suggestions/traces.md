# Traces: follow-ups

Traces lists the tests of a comparison and walks the calls below one test
top to bottom (`core/src/analysis/test_call_stack.rs`, `src/traces/`).
Things it does not do yet, in rough order of value. None are planned.

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
A "changed only" switch in the picker would then show the tests worth
walking first.

## Walking aids

- Keyboard: `j`/`k` or arrows to move between frames, `n`/`p` between changes.
- Fold runs of unchanged frames, like diff context.
- Show the calling line inline, so the walk reads as "line 11 calls ...".
- Open a frame's file in the Ledger view of a review of the same comparison.

## Working tree and large repos

The walk reads whole files from git once per file, so a stack through hundreds
of files is slow on a commit comparison. A text cache per index would fix it.

## Canvas version

The prototype also drew the frames on a ReactFlow canvas with the same three
weights, node heights computed from the diff line count so the layout never
measured the DOM. It was left out; the HTML page covers the MVP.
