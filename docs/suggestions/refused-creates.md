# Keep a refused create for the person

**Suggestion, 2026-11-27. Required by the outcome rule of
`docs/design/backend-protocol.md`: a refusal is shown, never dropped.** Found reviewing how a device handles refused
events, see `push_batch` in `core/src/sync.rs`.

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
