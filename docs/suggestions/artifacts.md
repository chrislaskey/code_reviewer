# Artifacts: the server serving them itself

Kept as on main through the move onto Starter Pack (2026-11-24): the Svelte
dev server serves `artifacts/` at `/artifacts/` (`svelte/vite.config.js`),
and the server finds them only when `bin/dev` starts it in the main
checkout, at the address `bin/dev` passes as `PAGES_DEV_URL`
(`core/src/app/sessions/artifacts.rs`). So without `bin/dev`, as for a
server run on its own or on another machine, a session has no artifacts and
pasted images can't be saved.

Suggested:

- The server serves the artifacts at `/artifacts/` itself, from a folder in
  its config, so they work wherever it runs. Safe now that the host routes
  take a bearer token alone: an artifact page on the server's address can
  reach no route that runs code.
- Pasted images saved in the server's data directory, not in the artifacts,
  so a session can take them without `bin/dev` too.
