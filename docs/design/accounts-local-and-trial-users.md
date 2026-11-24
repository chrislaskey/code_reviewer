# Using the app before signing up: local users and trial users

**Status: agreed and built 2026-11-11, see the build plan.** Since
2026-11-22 the pages call a trial user a **guest**: "Continue as guest"
on the Log in page starts one at once, and in place of the banner on
every page a dot on the avatar points to the account menu, which says
how long is left with Sign up (`svelte/src/lib/AccountsGuestNotice.svelte`,
on the profile page too); where the scenarios below speak of the
banner, read this. See `accounts-sign-in.md`. This document keeps the scenarios and the reasons.
What to build, as steps with what each must do, is
`accounts-local-and-trial-users-build.md`, and where the two differ
that one is right.

Two ways into the app with no sign-up:

- **A local user**, on desktop and mobile: "Skip" on the accounts page
  makes a user on this device only, named `local_85a3ycZae3b`, with the
  email `local_85a3ycZae3b@example.com` and no password. The app opens
  as that user at once. Nothing they make is queued as an event, and
  the device never pushes or pulls. Their profile says the user is tied
  to this device, with a way to sign up, which moves their records to
  the new synced account and removes the local user.
- **A trial user**, on the web: one click makes a temporary user on the
  server. The page says it lasts 24 hours; the server deletes it and
  everything of its, outright, 48 hours after it was made, unless the
  person finished signing up first.

No merging of two accounts, in either.

## Scenarios: the local user

**L1. First launch, Skip.** The accounts page shows Sign in, Sign up
and Skip. Skip needs no network: the app makes the user and their
database file, opens it, lands on home. Works on a plane.

**L2. Months of use.** Posts, files, perhaps communities. App updates
migrate the file like any account's. The nav shows nothing about sync:
no pending count, no "Working offline", no conflicts link.

**L3. The profile.** A banner: "This account is only on this device.
Nothing is backed up. Sign up to keep it safe and use it on your other
devices." The person can still change their name.

**L4. Sign up from the banner, the good path.** The browser opens the
sign-up page, the person enters a name, an email, the code, and comes
back with the device code as any sign-in does. The app moves every
record to the new user, removes the local user, and the first sync
pushes everything. A year of data is a long first sync: the nav shows
it counting down, and it resumes if cut short.

**L5. Sign up, abandoned halfway.** The person confirms their email in
the browser, then closes it before pasting the device code, which is
good for five minutes. The server now has an account with that email
and nothing in it. They cannot sign up with that email again, but they
can sign in with it. So this is L6: whether a sign-in brings the local
records along.

**L6. The second device.** The person signed up on their laptop a
month ago. They install the phone app, press Skip to look around, make
things for a week, then want their account. Their account exists, so
this is a sign-in, not a sign-up. Either the week's records move into
it, or they stay behind in a local user the person can only keep
beside the real one or remove. See To decide 2.

**L7. A real account beside the local one.** "Add account" signs a
synced account in as today. The accounts page lists both; the local
one says "On this device only". Switching works as between any two.

**L8. Skip, twice.** A second Skip would make a second local user. Two
people sharing a tablet might want that; one person pressing it by
mistake gets an empty app and thinks their data is gone. See To
decide 3.

**L9. Remove from device.** For a synced account this is safe, the
data comes back at the next sign-in. For a local user it is the only
copy. The confirmation must say so, and there is no sign-out for a
local user, since there is nothing to sign back in with.

**L10. The device is lost, reset or replaced.** The data is gone, as
the banner said. On a phone the system's own backup may carry the app's
folder to the new phone, in which case the local user arrives with it;
nothing we do, but it should not break.

**L11. Support, forum, account settings, a picture.** All of these are
online only and need a token, which a local user has not. The nav
either hides them or shows them with "Sign up to use this". A picture
is uploaded to the server's bucket, so a local user has none.

**L12. Communities.** There are no other people on a device that never
syncs, but some apps need a current community for every action. So a
local user starts with a community of their own, can create more, and
is the only member of each; the move pushes them like any row. See To
decide 5 and 10a.

**L13. The server says no at sign-up.** Rows made over months meet the
server's rules for the first time in one push: a validation that
changed, a plan's limit. Those rows land on the conflicts page as any
refused change does; the rest go through.

**L14. Sync costs money.** In some apps the free tier is the local
user and syncing is the paid plan. Then signing up alone should not
start a sync: there is a person with a real account whose device still
does not sync. That is a second, separate check, the plan, not a third
kind of user. See To decide 9.

## Scenarios: the trial user

**T1. "Try it now" on the public website.** One click, and the person
is in the app as `guest_85a3ycZae3b`. A banner on every page: "This is
a trial. Sign up within 23 hours to keep what you make."

**T2. They close the browser and come back after lunch.** Today's
cookie lasts the browser session, so the trial would be gone, with no
way back in: a trial user has nothing to sign in with. The trial's
cookie has to last as long as the trial. See To decide 11.

**T3. Sign up, the good path.** From the banner: a name, an email, the
code. The same user becomes a real account; nothing is moved, since
every row is already theirs on the server. The banner goes.

**T4. Sign up with an email that already has an account.** The quiet
answer as always, and that account's owner is mailed. No merge: the
person can sign in to their account, which leaves the trial behind to
expire.

**T5. Sign up started, code never entered.** The trial stays a trial;
the pending code expires on its own.

**T6. Hours 24 to 48.** The page said 24 hours. Past that, either the
app still works with a louder banner, or it shows only "Your trial
ended. Sign up before <time> to keep what you made." See To decide 12.

**T7. After the delete.** The cookie names a user that is gone: a 401,
and the public home page, ideally saying the trial ended.

**T8. Other people.** A trial user who posts in a community real
members are in puts rows on those members' devices. A hard delete 48
hours later leaves no deleted row for those devices to pull, so they
would keep the posts forever. Either a trial user's rows are seen by
nobody else, or the delete cannot be outright. See To decide 10.

**T9. A script presses "Try it now" ten thousand times.** Each press
is rows in the server's database with no email behind them, and if a
trial user can add files, a stranger's bytes in our bucket for 48
hours. See To decide 13.

**T10. The apps.** A trial user cannot sign in on a device, having no
email. Their path is: sign up on the web, then sign in on the device.

**T11. Staff.** The users list in ops marks trial users, and the count
of trials started against trials signed up is the number a product
person will ask for first.

## To decide

1. **The name.** "Offline only" collides with two things the app
   already says: "Working offline", the switch in the nav, and a synced
   account with no network. **Assumed for now: local user** in the
   code, the docs and the pages ("On this device only" where a label
   is needed), matching the `local_` in the name, and **trial user** on
   the web; to revisit if it stops fitting.

2. **Whether a sign-in, not only a sign-up, brings a local user's
   records along** (L5, L6). **Agreed: yes.** The worry
   with a merge is app-specific rules that do not mesh. Why it should
   not arise here: a local user can touch nothing outside their own
   device, so every row they made has an id the server has never seen,
   and they were the only member of anything they made. The move is
   therefore not a merge the app has to write. To the server it is an
   account whose device made 214 things while offline and now pushes
   them, which every app built on the template already has to handle:
   each create meets the server's ordinary rules, and one the server
   refuses lands on the conflicts page like any refused change.
   What that leaves for an app to think about is rules that count per
   user: "one of these per user", a plan's limit, a unique name. An
   account that already has its one, or is at its limit, gets those
   creates refused, and the person sees them on the conflicts page.
   That is the same thing that happens to a brand-new account whose
   local user went over the limit, so sign-in adds no case sign-up
   lacks. The one row that does compete is the user's own, name and
   picture: the account's wins and the local user's is dropped.
   Merging two accounts the server knows stays out.
   The surprise to guard against is signing in to the wrong account,
   so the app confirms before moving: "Move 214 items into
   alice@example.com?", with "Keep them separate" leaving the local
   user as it is and adding the account beside it.

3. **How many local users a device may have** (L8). **Agreed: one.**
   Skip shows while there is none; after that the accounts page lists
   it and Skip is gone.

4. **Whether Skip exists is the app's choice**, as
   `accounts::FORGOTTEN_PASSWORD` is: a constant in core each fork
   sets, one for local users and one for trial users. Recommended, both
   on in the template.

5. **Communities for a local user** (L12). Recommended: the same app as
   a signed-up user sees, communities included, so there is one app to
   build and test, and their communities are pushed at sign-up.

6. **What a local user's delete does.** A synced row is soft-deleted so
   other devices see it go. Nobody else will ever see a local user's
   row. Recommended: the same soft delete, so no write path differs,
   and the move at sign-up drops deleted rows rather than pushing them.

7. **The name after sign-up.** The sign-up page asks for a name.
   Recommended: that name wins, whatever the local user was called.

8. **The times on moved rows.** Checked in the code: on a create the
   server sets `inserted_at` to its own clock at the moment it applies
   the event (`posts::apply_event`), so a year of local posts would
   all read as made on the day of the move, and lose their order. The
   same already happens, smaller, to a synced user's post written
   offline on Monday and pushed on Friday. Recommended: on a create
   the server takes `inserted_at` from the event's `made_at`, the
   device's clock when the person made the row, unless that is later
   than the server's own clock, in which case the server's. The move
   queues each create with the row's `inserted_at` as its `made_at`.
   A change to the sync for every create, so to agree before building.

8a. **A push in batches.** Checked in the code: a push sends every
   pending event in one request (`sync::push_pending`), which a year
   of rows would outgrow. Recommended: a push sends at most 500 events
   a request, oldest first, and repeats until none are left; each
   request is answered and applied on its own, so a push cut short
   resumes where it stopped.

9. **Whether a device syncs is two separate questions**, not three
   kinds of user (L14): does this user have an account on the server,
   and does their plan include sync. A local user fails the first. The
   second is purchasing's, built with it; nothing here models the
   local user as a plan, and the sync asks each on its own.

10. **A trial user is seen by nobody else** (T8). **Agreed.** A trial
    user cannot join a community, nobody can join theirs, and their
    communities are in nobody else's list or pull. Then no device ever
    holds a row of theirs, and the outright delete is safe for the
    reason deleting an unconfirmed sign-up is. They also do not post in
    the forum or open a support request.

10a. **Every new user starts with a community of their own.**
    **Agreed, for every sign-up, regular ones included.** Some apps
    need a current community for every action, and the less the app's
    code knows about kinds of user the better. It is made at the
    moment the user is made, an ordinary community with the user as
    its one member, and nothing marks it as special. Its name says
    where it came from, so communities that end up in one account
    tell themselves apart:
    - a regular sign-up: `Personal`;
    - a local user: the device's name, "Chris's MacBook Pro";
    - a trial user: `Trial`.
    Nothing is renamed for anyone. A local user who signs up has both
    `Personal`, made by the sign-up, and "Chris's MacBook Pro" holding
    what they made locally; a trial that signs up keeps `Trial`. They
    rename or delete as they like, with the history plain to see.
    **Since 2026-11-24, on a server with a single community** (its
    settings' `communities`, see "Server settings" in
    `multi-server.md`): the local user's communities fold into the
    account's at the move, so the account stays in one, the pages
    showing no other. Before the move the device asks the server, which
    the sign-in just reached, for its settings, the build's defaults
    being all a device that never synced holds, and when they say
    `single`, for the account's communities; the one the account joined
    first is kept (`AccountCommunity::first_joined`), the same on each
    of its devices, Personal for a sign-up. In the move's transaction,
    `local_users::fold_communities_in` writes that community and the
    account's membership as the server has them, puts every row of the
    local user's communities in it (each synced table's
    `move_to_community`; a row in two keeps one), removes those
    communities, which never reached the server, and makes it the
    current one; the creates then queued leave out the two rows the
    server has. Nothing is merged on the server, and no community is
    deleted there. An account in no community keeps the device's. A
    later way back to a device alone, when the person stops syncing,
    would make the device's community again then, see
    `docs/suggestions/accounts.md`.
    To decide:
    - where the device's name comes from. Desktops give the name the
      person set. iOS since 16 answers only "iPhone" to apps without a
      special entitlement, and Android the model. Recommended: use
      what the system answers, and "This device" when it answers
      nothing. Needs a small crate or Tauri's os plugin, to ask about
      when building.

11. **Staying signed in, built first, and the trial's cookie on top
    of it** (T2). **Agreed in outline.** Today
    (`axum/src/accounts.rs`, `token_cookie`): one cookie, `token`,
    `HttpOnly`, `SameSite=Strict`, `Secure` except on localhost, path
    `/`, and no `Max-Age`, so the browser drops it when it closes.
    Behind it is a `session` row in `accounts_tokens`, good for sixty
    days on the server. First, for every user:
    - "Keep me signed in on this browser", a checkbox on the web's
      sign-in and sign-up pages. The page holds the choice and sends
      it with whichever request finishes the sign-in: the code, the
      password, the new password or the reset.
    - Checked, the cookie gets a `Max-Age` of the session token's
      sixty days; unchecked, it is as today. Nothing changes in the
      tokens table: the row was already good for sixty days.
    - Not shown when a device started the sign-in, which sets no
      cookie.
    - To decide: checked or unchecked to start with. Recommended:
      unchecked, for shared computers, as a constant a fork changes.
    - Left out: renewing the sixty days while the person keeps coming
      back, and skipping the emailed code on a browser that was
      remembered, which is a different thing with a similar name.
    Then the trial is the same code path with the choice always on and
    the duration 48 hours:
    - when the trial signs up, the cookie is set again as the sign-up
      page's checkbox says;
    - "Try it now" with a live trial's cookie goes back into that
      trial, and with a real session goes to the application; it
      never makes a second user;
    - a cleared cookie, a private window or another browser loses the
      trial; the banner's "sign up to keep what you make" covers it.

12. **24 hours shown, 48 hours kept** (T6). **Agreed.** The banner,
    on every page of a trial user and with no way to close it, counts
    down the 24 hours. Past them nothing else changes: the pages work
    as before and the banner reads "Your trial ended. Sign up before
    <time> to keep what you made." At 48 hours the rows are deleted.
    No page is closed to them in between, so there is no third state
    to build.

13. **Abuse of trials** (T9). **Agreed.** A trial is the one place a stranger
    makes rows with no email behind them. What could go wrong, and the
    answer proposed for each:
    - *Crawlers and link previews starting trials*: "Try it now" is a
      POST from a button, never a link.
    - *A script filling the database*: a limit per IP on the route,
      with the limiter the accounts routes have, starting at ten an
      hour; too low and a classroom or an office behind one address
      is locked out. And since addresses are cheap to an attacker, a
      cap on trial users alive at once across the deployment,
      `TRIAL_USERS_MAX`, starting at 1,000: past it the button answers
      "Trials are full right now, sign up instead". Every trial is
      gone in 48 hours, so the cap bounds the damage whatever the
      source, and 0 turns trials off without a deploy.
    - *Our bucket hosting a stranger's files*: no files and no picture
      for a trial user in the first version.
    - *Reaching other people*: walled off by 10, so nothing a trial
      user writes is shown to anyone. A fork that adds anything which
      emails or notifies others, an invitation say, must refuse it to
      a trial user; worth a line in the fork's checklist.
    - *Signing a victim's browser into a trial the attacker watches*:
      the route does nothing when the browser already has a session,
      and the cookie is `SameSite=Strict` as today.
    - *One person taking trial after trial instead of signing up*:
      accepted; each starts empty.
    - *Still open*: a CAPTCHA such as Turnstile in front of the
      button. Proposed: not in the first version, a suggestion for
      forks that see abuse.

    **Agreed 2026-11-11**, replacing the numbers above; what to build
    is step 5 of the build plan:
    - *The cap starts at 100, not 1,000.* The most a deployment can
      be made to hold is the cap, times the writes a trial may make,
      times the size of a write. With the two limits below, 100
      trials are about 500 MB at the very worst, gone in 48 hours.
      Unset stays off, so whoever deploys picks the number; `bin/dev`
      sets 100.
    - *Ten an hour per IP stays*, in a bucket of its own. It stops a
      runaway script, not a determined person: ten an hour is 480 in
      48 hours, enough to keep the cap full. Accepted, since that
      costs trials and never sign-up.
    - *One trial writing without end* was the gap: the cap counts
      users, not what they write, and a post's body had no maximum.
      So maximum lengths for everyone, 10,000 characters a post and
      80 a community's name, and 500 writes in a trial's life,
      counted from the events the server already records per user. A
      total and not a rate, because a rate bounds speed and a trial
      has 48 hours.
    - *A trial user lists only their own communities.* They could
      join none of the others, and listing them showed every
      community's name to someone with no email.
    - *The client IP behind a proxy* was the first entry of
      `X-Forwarded-For`, which the client can write, so every per-IP
      limit could be skipped. It becomes the last, the proxy's.
    - *Staff learn trials are full* from the log, where the refusal,
      a 503, is written as an error.
    - *A push fits the request the server takes.* Found on the way: a
      push was 500 events whatever their size against the 2 MB a
      request may be, and one over it was refused whole and sent
      again unchanged for ever. With a body of up to 10,000
      characters that is easy to reach, so the server takes 5 MB on
      the push and a device fills a request to four fifths of it.
    - *Not taken*: a rate limit per user on the application's routes.
      It bounds speed and not how much is stored, and with the 500
      writes adds nothing for trials; in `docs/suggestions/`, with
      the CAPTCHA.

14. **How often the delete runs.** **Agreed: an hourly job**, one more
    a `RecurringJob` in `core/src/jobs.rs` beside the purge and the sweeps, run
    by the same loop and by `app_web jobs run`, see `jobs.md`.

15. **Build order.** Recommended: staying signed in (11); a community
    for every new user (10a); the local user, since it is the free
    tier; the trial user, which is smaller and rests on the first two.

## Left out, for `docs/suggestions/accounts.md` once this settles

- Exporting or backing up a local user's data without signing up.
- Going back from a synced account to a local one.
- Files for trial users, with a quota.
- Extending a trial.
- A CAPTCHA in front of "Try it now".

## Settled after the list above was written

- 4: Skip and "Try it now" are each a constant a fork sets.
- 5, 6: a local user has the whole app, communities included, and the
  same soft delete; the move drops deleted rows.
- 7: the account's name wins.
- 8, 8a: agreed, as long as each stays as small as written; the batch
  is the standard push's, not something of the move's.
- 10a: nothing is renamed; "iPhone" as a device's name is fine.
- 11: "Keep me signed in" starts checked.
- L11: the online-only apps' links stay, and each app says it needs an
  account.
- 15: staying signed in; a community for every new user; the local
  user with the two sync changes; the trial user; abuse.
