# Sessions: left out of the MVP

Ideas and template features not in the rewrite. None are planned.

- A settings form to correct the detected shell, terminal app and tmux path,
  as the Starter Pack's tmux example had, and to change the prefix of the
  sessions the app creates (`cr` now, a constant in `sessions.rs`). The app
  detects them once per run now; restarting it detects again.
- The per-pane rows the template showed: each window and pane with its
  state (at a prompt, running, exited), its processes and the git status of
  its directory. The list shows one Claude Code status per session now.
- A list of every running Claude Code session, in tmux or not, as the
  template had under the tmux list. Only those in the app's sessions show.
- Showing every Claude Code session in a tmux session with several (only the
  most recently updated one shows), and the git branch of its directory.
- Start a session from a review: open Claude Code in the reviewed repository.
- Kill a session from the list (there is no delete yet), and remove its
  worktree and branch with it when they have no changes.
- Choose which commit or branch the new worktree's branch starts from; it is
  the repository's current HEAD now.
- Show the branch of each session's worktree on its card, read with git.
- Choose the directory with a folder picker instead of typing the path.
