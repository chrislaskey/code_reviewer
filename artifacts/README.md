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

`bin/dev` serves them at `/artifacts/` on its dev server, in dev only:
`/artifacts/` lists every project and worktree, `/artifacts/<project>/`
and `/artifacts/<project>/<worktree>/` the files below them, newest first.
When Tailscale is up, `bin/dev` prints the address a phone on the tailnet
opens. Run in the main checkout, it also tells the desktop app where they
are, for each session's Open › Open artifacts.

A worktree's artifacts are deleted with it: by the app's Merge and Close,
and by `bin/worktree merge` and `close`.
