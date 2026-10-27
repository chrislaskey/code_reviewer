# Reviews: left out of the MVP

Ideas and prototype features not in the rewrite. None are planned.

## From the prototypes

- The file-level views on the canvas: chips and blocks around a file card,
  with one edge per caller and call (`existing/frontend/src/explorations/`).
- The "Folded" choice for calls into dependencies and the standard library:
  one "+N in dependencies and stdlib" line instead of dimming or hiding them.
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

## Rust analysis

- Name crates by their Cargo package name instead of their directory, read
  from each `Cargo.toml`, so a path into another workspace crate matches
  exactly rather than by its tail (`find_function_in_other_crate`).
- Resolve method calls on values by type where the source makes it plain:
  a `let x = Foo::new()` or a `x: &Foo` parameter in the same function, so
  `x.total()` names `Foo.total` without the unique-name rule.
- Calls written as `Type::method(&value, args)` (UFCS) have one argument
  more than the method's arity and do not match; count the receiver when
  the path names a method.
- Record macro invocations (`my_macro!()`) as calls to a project macro, and
  functions that `macro_rules!` or derive macros generate.
- Show the `impl` header (trait and type) of a method in the ledger, since
  `from/1` alone does not say which `From` it is.
- Link trait method calls to every implementation when the receiver's type
  is a `dyn Trait` or generic bound, instead of dropping them as ambiguous.
- Title a Rust file's ledger card by the file's module (`core::analysis::rust`)
  instead of the module of its first changed function, which is the type of
  the first changed method (`core::analysis::rust::RustAnalyzer`).
