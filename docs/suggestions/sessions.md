# Sessions: left out of the MVP

Ideas and template features not in the rewrite. None are planned.

- A settings form to correct the detected shell, terminal app and tmux path,
  as the Starter Pack's tmux example had, and to change the prefix of the
  sessions the app creates (`cr`), the directories looked in for worktrees
  (`~/code`), the command Continue types (`claude --continue`) and the
  branch Complete merges into (`main`), all constants in `sessions.rs` now. The app detects them once per run;
  restarting it detects again.
- The per-pane rows the template showed: each window and pane with its
  state (at a prompt, running, exited), its processes and the git status of
  its directory. The list shows one Claude Code status per session now.
- A list of every running Claude Code session, in tmux or not, as the
  template had under the tmux list. Only those in the app's sessions show.
- Showing every Claude Code session in a tmux session with several (only the
  most recently updated one shows), and the git branch of its directory.
- Start a session from a review: open Claude Code in the reviewed repository.
- Kill a tmux session from the list without touching its worktree (both
  Complete choices remove the worktree).
- Rebase a session's branch on the main branch from the list, for when
  Merge into main refuses because the main branch moved on.
- Choose which commit or branch the new worktree's branch starts from; it is
  the repository's current HEAD now.
- Remember the command a session was created with, so Continue can type it
  again instead of `claude --continue`.
- Look deeper than the first level of the watched directories for
  repositories, or follow `git worktree list` from any one repository.
- Choose the directory with a folder picker instead of typing the path.
- More Review choices on a card: one commit of the branch, or the changes
  since the last review. The links are made in `ReviewMenu.svelte` through
  `routes.js`, so a choice is one more `reviewShowUrl` call.
- The other direction: a session from a review (start or continue the
  worktree of the repository being reviewed), through `sessionsIndexUrl`
  or a `sessionNewUrl({ repository })` with the form filled in.
