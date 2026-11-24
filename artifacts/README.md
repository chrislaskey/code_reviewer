# Artifacts

Files made while working, such as screenshots, page previews and notes, to
look at from a browser, a phone's included. They are kept in the main
checkout, whichever worktree made them, so the app run from the main
checkout sees every worktree's:

    artifacts/<project>/<worktree>/<file>

`<project>` is the main checkout's folder name, `<worktree>` the worktree's
(the same as its session's), or `main` for work in the main checkout
itself. `bin/artifacts` prints the folder for the current checkout,
creating it. Everything here but this README, `index.html` and
`.gitignore` is ignored by git, so artifacts are never committed.

The web server serves them at `/artifacts/`, run from the main checkout or
any worktree: `/artifacts/` lists every project and worktree,
`/artifacts/<project>/` and `/artifacts/<project>/<worktree>/` the files
below them, newest first. Under `bin/dev` the Svelte dev server sends
`/artifacts/` to it too, and a phone reaches it at the tailnet address
`bin/dev` prints. Each is sent sandboxed, so an artifact's scripts can't
act as the signed-in pages.

A worktree's artifacts are deleted with it: by the app's Merge and Close,
and by `bin/worktree merge` and `close`.
