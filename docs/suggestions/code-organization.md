# Code organization: suggestions

## Record the event inside the data write

Today a write to a synced `app_` table is safe only when it goes through the app module's `apply_event`, which records the event; the `data` write itself is `pub(crate)` and trusts its caller. In Elixir the low-level context would do what must happen on every change itself. Doing so here would make `data::app_posts::create` record its event, so no caller, in core or out, could write a synced row without one, and the data writes could become public like the rest.

It reworks the offline-sync write path (`docs/design/offline-sync.md`): the role's part (record on the server, queue on a device), the answer and its conflicts, and replication writes, which must not make events. Worth its own design once the code organization is done.
