# Communities: what was left out

Built in the port of `phoenix/docs/design/multi-tenancy.md`: communities,
memberships with join, leave and rejoin, a post in the communities its
owner chose, visibility, backfill and prune, see
`docs/design/offline-sync.md`. Left for later, on purpose:

- **A community's posts and members on its own page**, which shows
  only its name, Join or Leave, and a member's Edit and Delete today.
  `posts::list` already answers everything a user sees with each post's
  communities; a feed of one community is a filter on the
  same visibility predicate, and the members are the live memberships of
  the community, which a device holds once it is in it.
- **Roles beyond `member`.** `role` is a string column every row holds
  `member` in; `admin`, and whatever an admin may do, such as removing
  another member's post from the community, is an app's rule.
- **Who may edit or delete a community.** Any live member renames or
  deletes one today, `member` being the only role; there is no owner
  column. An app with admins narrows that to a role, in
  `communities::apply_in`, the one place both the server and a device
  check it. A community whose last member left can be changed by nobody
  until someone joins again.
- **Undoing a community's delete.** A delete takes every membership
  along at one moment, so reviving them would mirror a rejoin; nothing
  revives a community today.
- **Discoverable and public communities.** Every user sees every
  community of the deployment, which is what lets a device list and join
  them. An app that wants private communities needs a rule for who sees
  which, in each table's `visible_to!` predicate.
- **A user's name or profile.** `app_users` holds an email; the pages
  show it as the author.
- **Registering, logging in, logging out, deleting an account.** Every
  deployment acts as the dev user, see "Who the user is" in
  `README.md`.
- **The event log on a device is kept forever.** Applied events are
  deleted from a device's queue, so only conflicts stay; the server's
  log grows with every write, and the purge removes only the events of
  rows that are gone.
- **An app without communities.** Communities are the template's
  sharing model, see `docs/design/template-and-apps.md`, so an app that
  has no use for them still runs through one. A setting that makes
  sign-up join every user to one shared community, and hides the
  community pages, would let such an app ignore them.
