# Multi-tenancy

How to lay out tenancy so one codebase can serve three shapes of application:

1. **Isolated.** Each tenant is a sealed box. Nothing is shared or duplicated
   across tenants. Classic SaaS.
2. **Cross-community.** A user creates a resource once and chooses which of
   their communities it is visible in, per resource. A post in two of three
   communities.
3. **Mixed.** A public area where resources can be visible across many
   communities, and private enterprise areas that behave like sealed boxes.

Constraints that shape the answer:

- Mobile and desktop clients are offline-first and sync to the server. See
  `offline-sync.md`.
- The current adapter is SQLite (`ecto_sqlite3`). Postgres is plausible later
  and nothing here should rule it out.
- All database access goes through `App.Data.Context`.

## Decisions so far

Settled in discussion. Everything else in this doc follows from these.

- **A tenant that needs isolation is its own deployment.** One deployment is
  one server and one database, with its own users. Nothing references
  across deployments. Isolation is physical: separate process, disk,
  backups, keys and region, which is what enterprise reviews and data
  residency rules ask for.
- **Inside a deployment, communities are the boundary.** Users join
  communities, and a resource is visible in the communities its owner chose.
- **A person in two deployments has two accounts.** A business that wants
  its data separate wants its identity separate too, usually through its own
  identity provider. When one login across deployments is needed, it comes
  from SSO over OIDC, not from shared tables.
- **One database per deployment, namespaced by table prefix.** `users` and
  `users_tokens` are the accounts. Everything the app owns is prefixed
  `app_`: `app_users`, `app_communities`, `app_posts`. Another product in the
  same deployment, such as a forum or support desk, adds its own prefix,
  `forum_posts` or `support_tickets`, and shares the accounts.
- **`app_users` is the app's view of a user.** Same id as `users`, written
  at registration, holding only what the app shows. Every foreign key of an
  `app_` table points at it, never at `users`. It syncs to clients; `users`
  holds credentials and never leaves the server.
- **The scope is for authorization, not routing.** `App.Web.Scope` carries
  `user` and, inside a community, `community`. Data contexts take no scope;
  flows such as `App.Data.Posting` take it first and check the user against
  it.
- **Ecto migrations are the schema source of truth.** Client SQLite
  migrations are generated from them, like the Svelte mirrors. Datetimes
  and sync transport are settled in `offline-sync.md`.
- **Resources have one clear primary owner.** Conflict resolution is last
  write wins. Permission rows are server-wins.
- **Soft deletes everywhere.** `deleted_at` and `deleted_by_id`, with a purge
  job after a retention window.
- **UUIDv7 primary keys everywhere.** Time-ordered, client-generatable, and
  supported natively: Ecto 3.14 via `autogenerate: [version: 7]`, Postgres 17
  via `uuidv7()`, the Rust `uuid` crate with the `v7` feature, and the `uuid`
  npm package from v10. SQLite stores them as text or 16-byte blobs with no
  special support needed. With no reference across deployments and ids that
  never collide, moving or merging tenants stays a mechanical data copy.
- **A post is in zero or more communities, chosen by its owner.** The
  owner adds their own post to a community or removes it, and nobody else
  can put a post anywhere. There is no re-sharing of other people's
  resources.
- **No home community.** A community admin removing a post from their
  community removes it from that community only.
- **Leaving a community removes your posts from it.** All of a member's
  own posts leave the community with them. Other members' posts stay.
- **Soft-deleted state is restorable until purged.** Rejoining within the
  retention window revives the membership, the posts that left with it, and
  their comments. Adding a post back to a community it was removed from
  revives its earlier comments there. After the purge job has run there is
  nothing to restore, so the result is a fresh start.
- **Comments and reactions are per community.** A comment belongs to the
  post's presence in one community, not to the post globally.
- **A post in zero communities is valid.** It is visible only to its owner.
- **Communities are flat.** No nesting.
- **Roles are string columns with app-level enums, and nothing is gated
  yet.** `app_users_communities.role` exists as a string column backed by
  `Ecto.Enum` in the schema, so adding a value is a code change with no
  migration. Every row is `member` to start. Permissions, and the `admin`
  value, come later. The same rule applies to every enumerated column:
  never a database enum type or a check constraint.
- **Email is the unique identifier for a person.** One email, one account
  per deployment. Multiple credentials per account and email as a has-many
  come later.
- **Deleting an account soft-deletes everything, then the purge removes
  it.** The `users` row, its tokens, the `app_users` row, memberships, and by
  cascade their content, in one transaction. Personal data is removed by
  the ordinary 90-day purge like any other soft-deleted row. No
  immediate-purge path. `users` has `deleted_at` and a partial unique index
  on email, so the address can be registered again right away.
- **Leaving a community cascades to everything the member made there.**
  Their posts' community rows, comments, reactions, all of it, soft-deleted with one
  `deleted_at` value so a rejoin within the retention window restores it.
  No retain, anonymize, or orphan variants. An app that needs one builds it.
- **The template does not decide moderation rules.** Whether admins can
  edit other members' content is up to each app built from the template.
- **SQLite until an app approaches its limits.** A deployment is one file
  with one writer. Expect to move to Postgres before any app has enough
  active users to feel that.
- **Public communities are joinable by anyone eventually, not at the start.**

## Vocabulary

"Tenant" is overloaded: it means the sealed box in scenario 1 and the place
a resource is visible in scenario 2. This doc uses two words instead:

- **Deployment.** The hard boundary. Its own server and database. Nothing
  crosses it. An enterprise customer is a deployment. The whole public area
  is one deployment.
- **Community.** The soft boundary inside a deployment. Users join
  communities. A resource is visible in the communities its owner chose. One
  resource can be in several communities.
- **Membership.** A user's relationship to a community, with a role.

Under this vocabulary the three scenarios collapse into one model:

| Scenario        | Deployments                       | Communities per deployment | A resource can be in   |
| --------------- | --------------------------------- | -------------------------- | ---------------------- |
| Isolated        | one per tenant                    | one                        | that one community     |
| Cross-community | one                               | many                       | any of them            |
| Mixed           | one public, one per private tenant | varies                    | any in its deployment  |

A classic isolated tenant is a deployment with a single community. The code
path is identical in every shape.

## Storage patterns considered

### 1. Deployment per tenant (chosen)

One server and one database per tenant that needs isolation. Many small
tenants that need no physical isolation live as communities in a shared
deployment.

Pros

- Strongest isolation, by construction rather than discipline. No
  `where tenant_id` to forget, and nothing in the code knows about tenants.
- Export, backup, restore, or delete a tenant by copying or removing a
  database. Contractual "your data is physically separate" is true, down to
  the server.
- One repo, one migration run, one transaction for anything, real foreign
  keys from app tables to `app_users`.
- Mirrors the device exactly: a client holds one file per server and
  account, with the same tables as the server. See `offline-sync.md`.
- Performance isolation. A noisy tenant cannot slow another.

Cons

- A person in two deployments has two accounts, until SSO.
- One deployment per tenant is fine for a handful of enterprise tenants and
  costly for thousands of small ones. Those are communities in a shared
  deployment instead, isolated by the application rather than physically.
- Cross-tenant admin and reporting means querying every deployment.
  Acceptable, because the boundary is the point.

### 2. Database per tenant inside one deployment

Several tenant databases behind one server, with a shared identity database
and every query routed by a scope that names the tenant. Rejected: it buys
one login across tenants at the cost of a repo per tenant, routing in every
data call, migrations run per tenant, no transaction across identity and
tenant data, and a fan-out on account deletion. SSO gives the one login more
cheaply when it is needed.

### 3. Shared tables, tenant discriminator column

Every row carries a `tenant_id`. Rejected: it makes isolation a matter of
never forgetting a `where` clause, with no safety net on SQLite.

### 4. Shared tables, join table for visibility

Resources belong to an owner; a join table records which communities can see
each one. This is the model **inside** a deployment, and the recommended
layout below uses it.

#### Polymorphic vs per-resource join tables

A single `resources_communities(resource_type, resource_id, community_id)`
table is compact but cannot carry a foreign key to the resource, so orphan
rows are possible and every query needs a type filter.

A join table per resource type (`app_posts_communities`,
`app_files_communities`) has real foreign keys and simple queries, at the
cost of one more table per resource type. That matches this repo's
preference for repeated, bounded code over clever shared machinery, and the
code generators can emit the table and context together.

Recommendation: per-resource join tables.

## Recommended layout

One database per deployment:

```sql
-- Accounts, server only
users                   (id, email, hashed_password, confirmed_at, ...)
users_tokens            (id, user_id, ...)

-- The app, synced to clients
app_users               (id, email, joined_at, ...)   -- same id as users
app_communities         (id, name, ...)
app_users_communities   (id, user_id, community_id, role, ...)
app_posts               (id, owner_id, body, ...)
app_posts_communities   (id, post_id, community_id, users_communities_id, ...)
app_comments            (id, posts_communities_id, owner_id, body, ...)   -- later
app_reactions           (id, posts_communities_id, user_id, kind, ...)     -- later

-- The app, server only
app_purges              (id, horizon, inserted_at)
```

An `app_posts_communities` row says "this post is in this community" and
records the membership it was added under, `users_communities_id`. Since only
the owner adds a post to a community, that membership is always the
owner's. Comments and reactions hang off the `app_posts_communities` row,
not the post, which is what "per community" means in SQL.

Plus on every synced table, on the server and on every client:
`updated_at`, `last_synced_at`, `deleted_at`, `deleted_by_id`. Those columns
are the sync protocol and are specified in `offline-sync.md`.

Rules that hold it together:

- **`app_users` is the only user data the app's tables see.** One row per
  user, with the same id as their `users` row and only the fields the app
  shows. It is written at registration and its email kept in step on an
  email change. Every foreign key in an `app_` table points at it.
- **Join tables are named for both sides.** `app_users_communities`,
  `app_posts_communities`. A join table that carries meaning beyond the
  join, such as which membership added a post, keeps the naming and adds
  columns.
- **Visibility is membership plus presence, checked at different levels.**
  `app_posts_communities` says a post is in community C.
  `app_users_communities` says a person is in C. Membership is checked once
  when a request is scoped to a community. Inside that scope, listing posts
  is a join on `app_posts_communities` alone. Only queries that span
  communities, such as the sync pull's "everything visible to this user,"
  check both. Owners always see their own posts.
- **Leaving cascades through the membership, and only that far.** Every
  `app_posts_communities` row points at the membership it was added under.
  Leaving community C soft-deletes the member's `app_users_communities` row
  and every `app_posts_communities` row that references it, which is exactly
  their own posts in C. Comments and reactions under those rows disappear
  with them because reads join through the row. Other members' posts point
  at other memberships and are untouched. The purge job's hard delete
  follows the same path with `on delete cascade`. This is an application
  rule in the leave function on the server, not a database trigger, so the
  client receives the soft-deleted rows through sync like any other change.
  The leave function computes `deleted_at` once and writes the same value to
  the membership row and every row it cascades to, which is what makes the
  revive below precise.
- **Rejoining revives what leaving removed, and nothing else.** If a
  soft-deleted membership still exists, rejoining clears `deleted_at` on it
  and on every `app_posts_communities` row whose `users_communities_id`
  matches and whose `deleted_at` equals the membership's. Rows deleted at
  any other moment, by the owner on purpose or by an admin, were deleted
  for their own reasons and stay deleted. Comments and reactions under
  revived rows reappear because reads join through them. If the membership
  was already purged, rejoining inserts a fresh row and starts from nothing.
- **Join tables soft-delete like everything else.** `app_users_communities`
  and `app_posts_communities` carry `deleted_at`, because clients learn that
  a post left a community, or a user did, only by receiving those rows.
  Every visibility query filters `deleted_at is null` on the post and on
  both join tables, applied by the data context. Adding a post back to a
  community clears `deleted_at` on the existing row instead of inserting a
  new one, so ids stay stable and the unique index on (post, community)
  stays plain, and the earlier comments there return with it. Deleting a
  post copies its `deleted_at` onto its community rows, so no read has to
  check the post, see "Deletes" in `related-resources.md`.
- **No home community.** A post has an owner and is in zero or more
  communities, all equal. The owner deletes the post; a community admin can
  only soft-delete the post's row for their community.

## Phoenix

One repo, `App.Repo`, over the deployment's one database. Every table has a
data context made with `App.Data.Context`, which applies the soft-delete
filter on every read:

```elixir
defmodule App.Data.Posts do
  use App.Data.Context, schema: App.Data.Post, sorts: [desc: :inserted_at]
end

Posts.list(filters: [visible_to: scope])
```

Flows that span tables and check who may do what take the scope first, like
`Posting.write(scope, params, community_ids)` and
`Memberships.join(scope, community_id)`. `App.Web.CommunityAuth` puts the
community of a LiveView inside one in the scope after checking the
membership.

## Devices

An offline-first client mirrors the server: a small profile of the user,
from the `auth.user.get` request on the websocket API, plus one SQLite file
with the same synced tables as the server. Logging out deletes both. A client
used with two deployments keeps a profile and a file per server. Details are
in `offline-sync.md`.

## Open questions

One left. Everything else has moved up to "Decisions so far."

1. **What does the community-scoped resource generator produce?** The table, its
   `*_communities` join, optional comments and reactions, the context, the
   sync columns, the synced-table marker for the client migration export,
   the leave cascade, and tests. Decides how much of this design is a
   template convention versus hand-written per resource.

## Build order

Implementation order, phase by phase, is in `build-order.md`.
