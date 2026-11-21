# Code organization in core

Status: built 2026-11-21, steps 1 to 5 below.

## Request

Organize `core/` as the project owner's Elixir applications are:

- **data**: schemas and low-level contexts, one module per resource with the basic `list`, `get`, `create`, `update` and `delete`. Every write to a resource goes through its context, so logic that must run on each change, and baseline validation, live in one place.
- **services**: clients for other services, usually over HTTP: Stripe, the App Store, a mail provider.
- **helpers**: generic functions callable from anywhere: data transforms, hashing, URLs.
- **high-level contexts**: business logic touching many resources or services, shared by two or more endpoints. These need no directory of their own: the per-app directories (`app/`, `accounts/`, `ops/`, and later `support/`, `forum/`) already are them. An app that wants to group them further uses `<app>/contexts/`.

## Layout

```
core/src/
  lib.rs, database.rs, deployment.rs   the foundation: connection, transaction, migrations, deployment
  data/        schemas and low-level contexts, one per table
  services/    stripe, app_store, mailer (+ mails), storage, web_server (a device's client of our server)
  helpers/     field_errors, fields, sha256, random
  app/         the app's high-level contexts, jobs included
  accounts/    the accounts app's, trial_users included
  ops/         the ops app's
```

`database.rs` (with the embedded migrations) and `deployment.rs` fit none of the four, as Elixir's `Repo` and `Application` sit outside them; they stay at the root.

## Decisions

- **Import paths are renamed, not re-exported.** Callers name each module where it lives (`app_core::app::posts`, `app_core::services::stripe`); `lib.rs` keeps no compatibility re-exports.
- **`data` is public, schemas and functions both.** As in Elixir, any part of the application may read a schema or call a low-level context directly; going through a high-level context is not required. What the boundary prevents is SQL outside `data`: each context's SQLite and PostgreSQL code is a private submodule, so only the context itself can issue it.
- **Writes take an accepted row, made by the context's validation.** Each `data` write takes an `Accepted*` value (`AcceptedPost`) that only its context's constructor makes, `AcceptedPost::new(&post)`, after the baseline validation, as an Elixir changeset. Rules about who may write and which fields stay in the app's module. Writes to synced `app_` tables stay `pub(crate)`: the logic every one of them needs is recording its event, which `apply_event` does, so neither the server nor the app may write those rows directly. Reads, and writes to tables that never sync, are `pub`. A row with no rules of its own (`Membership`, `PostCommunity`, `FileCommunity`) has no `Accepted*` type; its writes take the row. A replication upsert takes the server's row as it is, the server being the system of record. A validation failure reaches the app's error as one variant, `PostError::Validation(PostInvalid)`, or, where the resource already reports by field, as `FieldErrors`. Recording the event inside the `data` write itself is in `docs/suggestions/code-organization.md`.
- **Schemas move into `data`.** `Post`, `Community`, `User` and the rest are defined beside their context, not in the app module that uses them. Shapes made for a page (`PostWithOwnerAndCommunities`, `ProductForSale`) stay with the high-level context that builds them. Baseline validation (`checked_body`, `normalized_email`, `checked_password`) moves with its schema.
- **A `data` module has its table's name**, prefix included: `data/app_posts`, `data/accounts_tokens`, `data/app_users_communities`, `data/device_settings`. One rule with no judgment, ready for a `forum_posts` beside `app_posts`, and `use crate::app::posts;` sits beside `use crate::data::app_posts;` without aliases. The exceptions are `data/local_users` and `data/trial_users`, bulk operations over several tables with no table of their own, see `low-level-contexts.md`.
- **Each resource's `apply_event` stays in its app's module.** Applying a synced event is business logic over several resources.

## Steps

1. `helpers/` and `services/`: pure moves, callers renamed.
2. Drop the root re-exports of `app::*`; move `jobs` into `app/` and `trial_users` into `accounts/`; rename callers.
3. Schemas and baseline validation into `data/`; rename `data` modules to their tables; fold `tokens`, `credentials`, `device_settings` and `accounts/prices` record code into their contexts; make `data` public.
4. Split `app/sync.rs`: the HTTP client to `services/web_server`, the rest stays; event types into `data/events`.
5. Update `AGENTS.md`, `multi-app.md` and `low-level-contexts.md`.

## As built

- A module that already was a table's low-level context moved into `data` whole, its functions becoming the context's: `accounts::users` (only the deployment's `get` stays there), `tokens`, `credentials`, `device_settings`. Their `create` and `get` check and normalize their input themselves, as an Ecto context does.
- An enum that names a column's values moves with its schema (`Job`, `CheckoutStatus`, `Store`, `SubscriptionStatus`, `PriceInterval`); methods on it that translate a service's types or decide policy stay in the app's module as a second `impl` (`CheckoutStatus::of`, `Job::interval`).
- `Cursor` is the schema of `app_sync_state`; `Event`, `StoredEvent` and the queue's reads and writes are `data/app_events`. Merging and answering an event stay in `sync::events`. `data` imports nothing from the apps or services.
- `services/web_server` holds the `Transport` a sync and a sign-in go through: `Http` for the apps, `InProcess` for tests.

## Open

- `tauri/src/device_accounts.rs` issues its own SQL against the device's account list; it stays outside this effort.
