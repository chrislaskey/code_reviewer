# Keep a refused create for the person

**Built, 2026-11-27.** Required by the outcome rule of
`docs/design/backend-protocol.md`: a refusal is shown, never dropped.
Found reviewing how a device handles refused events, see `push_batch` in
`core/src/sync.rs`. The design is now "Events" in
`docs/design/offline-sync.md`; this page keeps the reasoning.

## The gap

A create the server refuses, with no row of its own to send, is handled
in two ways:

- **A table with bytes, such as files**, keeps the create. It is marked
  refused, with the reason. The file shows as refused, and the person can
  choose Try again or Let go.
- **Any other table, such as posts**, drops it. The device deletes the
  row, its event and its pending edits. Nothing tells the person, so a
  post written offline can vanish without a word, if it was refused for
  being invalid or for a community the person has left.

## Suggestion

Keep every refused create as a refused conflict, as files are kept.
`conflicts::try_again` and `conflicts::delete` already exist for files.
The conflicts page would list it with its reason. The row would stay on
the device until the person lets it go.

Things to decide:

- **Should a refused create's row stay visible in its list,** marked as
  refused, as a file's does? Or should it show only on the conflicts page?
- **Are there reasons to drop silently?** For example `exists`, when the
  server already has the row under the same id.

## As built

- **The row stays in its list**, as a file's does. The person sees the
  conflict on the conflicts page, with the reason, and chooses Try again
  or Let go. A post's list doesn't mark it as refused; that would be a
  page change of the app's.
- **No reason drops silently.** When the server has the row, it sends
  it, and the server's row wins; that was already so.
- **What waits with it.** The row's later edits, and the creates of rows
  pointing at it by a foreign key (a post's rows in communities), are
  held back from each push, as a file's are. In the request that
  refused it, their answers are ignored and they stay pending.
- **A resync keeps it.** `delete_not_pending_or_refused` keeps a row
  whose create was refused.
- **A post's row in a community left meanwhile** is refused with
  `not_a_member`, and kept. Until the person lets it go, the device still
  shows the post in that community.

- **Updates, decided with the user the same day.** An update rejected
  with the server's row used to give way silently. Now the server's row
  wins and the person's change is kept as a refused conflict, to reapply
  or let go, and nothing waits on it. An update rejected without a row is
  refused at once, not tried 30 times: a rejection is final, and a
  temporary failure is a transport error. Only a file waiting for its
  bytes still counts tries.
- **A create rejected with the server's row** means the server already
  has a row under that id, such as `exists`. The server's row wins, with
  nothing to tell.

Open: should a refused row show a mark in its list, as a refused file
does? That is a page change, posts' own.
