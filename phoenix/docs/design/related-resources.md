# Related resources: links, tags, and deletes that copy

The spec for the second and third kinds of resource after a single post:
a post **has many links**, and posts and **tags** are **many to many**
through `posts_tags`, a schema of its own with fields people edit. It
also changes two things for every resource: a delete now copies onto the
rows below it, and the event log becomes one shared table that a
resource opts in to.

Companion to `offline-sync.md`, whose rules this follows unless it says
otherwise here. The flows are checked by
`test/app/data/sync/related_resources_test.exs`, written before the
implementation: when a test and this doc disagree, one of them is wrong,
and the disagreement is the thing to discuss.

Built on the server, in sync and on the web: `App.Data.EventFlow`
applies an event to any of these resources, `App.Data.Deletes` copies
deletes, `App.Data.Events` is the log, and `App.Data.Posting` and
`App.Data.Tagging` hold each resource's rules. The web's posts page and
`/tags` write with `App.Data.Sync.save/4`. The desktop app has the same
screens, writing through the same events, see `tauri/docs/offline-sync.md`.

## Decisions

- **Links** belong to a post and are owned through it: only the post's
  owner adds, edits or removes them.
- **Tags** are owned by a user, like posts. A name is unique per owner,
  ignoring case, among live tags. Two users can each have "elixir".
- **A tag goes only on its owner's own posts.** Someone who sees your tag
  on a post cannot put it on theirs. Otherwise a tag's name would follow
  their post into communities you are not in.
- **`posts_tags`** joins a post and a tag, with a `position` and a `note`.
  Only the post's owner, who is also the tag's owner, writes it.
- **Communities are a downstream link.** A tag reaches a community's
  members only on a post in that community. A `tags_communities` join,
  like `posts_communities`, is how "tags in this community" would work
  later. It is not part of this.
- **Every delete copies onto the rows below it**, with the same
  `deleted_at`, so every row says for itself whether it is live. Nothing
  is hidden through its parent any more, `posts_communities` included.
  See "Deletes".
- **The web writes through the same messages as a device**: it builds the
  events and rows a device would push and calls `App.Data.Sync.push/3`,
  so a rejection or conflict is in the answer it shows at once. No
  resource has a second, direct write path.
- **One event log, `app_events`, opted in to per resource.** See "The
  event log".

## Tables

Every table has the sync columns of `offline-sync.md`: `version`,
`last_synced_at`, `deleted_at`, `deleted_by_id`, `inserted_at` and
`updated_at`. Ids are UUIDv7, made by the device.

`app_links`

| Column | Rules |
| --- | --- |
| `post_id` | the post, never changed after create; `on_delete: :delete_all`, which only the purge reaches |
| `label` | required, at most 100 characters |
| `url` | required, an `http://` or `https://` URL with a host, at most 2000 characters, else "must be an http:// or https:// URL" |

`app_tags`

| Column | Rules |
| --- | --- |
| `owner_id` | the user who created it, from the scope, never a param; `on_delete: :delete_all` |
| `name` | required, trimmed, at most 50 characters; unique per owner ignoring case among live tags, a partial unique index on `(owner_id, lower(name)) where deleted_at is null` |

`app_posts_tags`

| Column | Rules |
| --- | --- |
| `post_id` | the post, never changed after create; `on_delete: :delete_all` |
| `tag_id` | the tag, never changed after create; `on_delete: :delete_all` |
| `position` | required integer, 0 or more; the tag's place on the post |
| `note` | optional, at most 200 characters |

A plain unique index on `(post_id, tag_id)`, deleted rows included: the
pair has one row for good, and tagging again revives it. A post's tags
show in `position` order, then by name. Positions may tie, see the
edge cases.

`app_events` is described under "The event log".

The changesets for links, tags and posts_tags are `@universal_changeset`,
so the desktop and mobile apps check the same rules in Rust.

## Writing

Links, tags and posts_tags change only through events, like posts. The
fields an event may set:

| Table | `create` | `update` |
| --- | --- | --- |
| `app_links` | `post_id`, `label`, `url` | `label`, `url`, `deleted_at` |
| `app_tags` | `name` | `name`, `deleted_at` |
| `app_posts_tags` | `post_id`, `tag_id`, `position`, `note` | `position`, `note`, `deleted_at` |

Any other field is refused with `not_editable: <fields>`. An update event
merges field by field exactly as a post's does.

Push order, parents first: `app_users`, `app_user_profiles`,
`app_communities`, `app_users_communities`, `app_posts`,
`app_posts_communities`, `app_links`, `app_tags`, `app_posts_tags`.

Refusals, each answered `rejected` with the reason and the server's row
when there is one:

| Reason | When |
| --- | --- |
| `not_owner` | a link or posts_tags row on someone else's post, or an edit to someone else's tag |
| `not_own_tag` | a posts_tags row with someone else's tag |
| `unknown_post`, `unknown_tag` | the parent does not exist on the server, deleted or not |
| `exists` | a create for an id the server has, or a posts_tags create for a pair the server already has, live or deleted; the answer carries the server's row |
| `invalid: ...` | the changeset refused it; the answer also has `errors`, each field's messages with their values filled in, such as `%{"url" => ["must be an http:// or https:// URL"]}`, which the web shows in the form |

A device that gets a create back `rejected` drops its own row; the
server's row in the answer, if any, takes its place. Re-tagging a post
whose tag was removed is an update clearing `deleted_at` on the row the
device already has, not a create.

## Deletes

**The rule: a row is never live while a row above it is deleted.** Every
write keeps that true:

- **Deleting copies down.** Deleting a row sets the same `deleted_at`
  and `deleted_by_id` on every live row below it, in the same
  transaction, each with its version incremented and `last_synced_at`
  set. Rows already deleted are left alone.
- **Undeleting copies down what the delete took.** Clearing a row's
  `deleted_at` revives the rows below it whose `deleted_at` equals the
  value cleared. A row deleted on its own, at another moment, stays
  deleted.
- **A row with two parents.** When one parent is undeleted and the
  other is still deleted, the row takes the other parent's `deleted_at`
  instead of reviving, so undeleting that parent later finds it.
- **A write under a deleted parent is kept, deleted.** A link created,
  or a posts_tags row revived, under a deleted post is applied with the
  post's `deleted_at`, so undeleting the post brings it back. Nothing the
  person wrote is lost.

What sits below what:

| Row | Parents |
| --- | --- |
| `app_links` | its post |
| `app_posts_tags` | its post and its tag |
| `app_posts_communities` | its post and its membership, `users_communities_id` |
| `app_comments` | their `posts_communities` row, see `comments.md` |

Deleting a tag removes it from its owner's posts and leaves the posts
alone. Deleting a post leaves its tags alone: a tag may be on no post.

The copies are not events a device pushes. A device copies locally too,
so its own screens hide the rows at once, and the server's rows replace
its copies on the next pull.

## The event log

One table, `app_events`, server only, never synced. A resource opts in;
one that doesn't still changes through event messages, merged by
version, but nothing is stored. Profiles never log: they are the example
of a resource that resolves conflicts by version with no history.

| Logs | Stream |
| --- | --- |
| `app_posts` | the post |
| `app_links` | its post |
| `app_posts_tags` | its post |
| `app_posts_communities` | its post: when and where the post was visible |
| `app_tags` | the tag |
| `app_user_profiles` | does not log |

A **stream** is what a rebuild would replay: replaying a post's stream
in order gives the post, its links, its tags and its communities.

| Column | Meaning |
| --- | --- |
| `id` | the event's id: the device's for a pushed event, the server's for one it writes |
| `stream_table`, `stream_id` | the stream |
| `sequence` | 1, 2, 3... within the stream, no gaps |
| `row_table`, `row_id` | the row the event changed |
| `user_id` | who made it, or whose action caused it |
| `type` | `create` or `update` |
| `base_version`, `version` | the row's version the change started from and the version after it |
| `changes` | what applied, so a replay gives the row |
| `conflicts` | what didn't, `{"field" => {"yours" => ..., "server" => ...}}` |
| `caused_by_table`, `caused_by_id`, `caused_by_version` | for an event the server wrote because another row changed: that row, at the version the change gave it |
| `inserted_at` | when the server recorded it |

- **Every change to a logged row is an event**, whoever made it: a
  pushed event, a server-wins row applied through its flow
  (`posts_communities`), and every copied delete or revive.
- **Caused by points at a row change, not an event**, so it works when
  the cause doesn't log: leaving a community writes an event into each of
  the member's posts' streams, caused by their `app_users_communities`
  row, which has no log.
- An event that applied nothing is still recorded, with the version
  unchanged, so a retry gets the same answer. `sequence`, not `version`,
  orders the stream.
- A retried event, by id, gets the recorded answer and applies nothing
  twice. A resource that doesn't log answers a retry by merging again,
  which can show a needless conflict.
- `app_post_events` moves into `app_events` and is dropped.

## Seeing and syncing

- A post's links and posts_tags are visible to whoever sees the post,
  deleted ones included, as tombstones.
- A tag is visible to its owner, and to whoever sees a post with a
  posts_tags row for it, live or deleted.
- A member's device learns a post left their community, or was
  deleted, from the `posts_communities` tombstone, and prunes the post
  with its links and posts_tags, the reachability pass of
  `offline-sync.md`. The owner's devices get every tombstone.
- A live `posts_communities` row carries, besides its post and the post's
  owner, the post's live links, live posts_tags and their tags, all
  placed before the rows that point at them. Backfill carries the same.
- The purge goes children first: posts_tags, links, posts_communities,
  posts, tags. When a stream's own row is purged, the stream's events go
  with it. A link purged on its own leaves its events in the post's
  stream.

## Edge cases and what to expect

Each is a test.

1. **A post is deleted on one device while another device, offline,
   edits one of its links.** The edit applies to the deleted link.
   Undeleting the post shows the edited link.
2. **A link is added offline to a post deleted elsewhere.** Applied,
   deleted with the post. Undeleting the post brings it back.
3. **A link removed before its post was deleted** stays removed when the
   post is undeleted.
4. **One person's two devices offline both create "Elixir".** The second
   is rejected, `invalid` with the name taken. Its posts_tags rows that
   point at its own copy are rejected `unknown_tag`, and the device drops
   both.
5. **Two devices tag the same post with the same tag.** The second gets
   `exists` and the server's row, and keeps only that.
6. **Two devices reorder the same post's tags.** Each position merges
   field by field. Two tags may end up at the same position; they show
   by name.
7. **Untag, then tag again.** The same row comes back with the new
   position and note.
8. **A post deleted, then its tag deleted, then either undeleted
   first.** The posts_tags row is live only once both are.
9. **A post deleted while its owner has left one of its communities.**
   Undeleting the post leaves that community row with the membership's
   `deleted_at`; rejoining brings it back.
10. **A tag renamed on one device while another, offline, renames it
    too.** The second is a conflict, as for a post's body.
11. **A tag's name on someone else's post.** A member sees it only while
    the post is in a community they are in, and sees renames.

## What changes in existing behaviour

- Deleting a post now soft-deletes its `posts_communities` rows too, so
  tests that expect them live after a post delete change.
- Undeleting a post revives them, by the matching `deleted_at`.
- Rejoining a community revives a post's row only if the post is live;
  otherwise the row takes the post's `deleted_at`.
- `app_post_events`, `App.Data.PostEvent` and `App.Data.PostEvents` go;
  the post's history reads from `app_events`.
- `App.Data.Posting.write/3` and `delete/2` build messages and go through
  `App.Data.Sync.push/3`, as the web does.

## Open

- A server-wins row applied through its flow is answered `rejected` with
  reason `server_wins` and the server's row, even when the flow did what
  was asked. The web treats that as success. A clearer answer, such as
  `applied` with the row, can come with the web UI.
- The web shows a tag's position and lets the owner add and remove tags,
  but has no screen yet to reorder them or edit a note; the events allow
  both.
- Tagging a post on the web places the tag after the post's live tags,
  by count, so after removals two tags can share a position. They show
  by name, as for ties from two devices.
