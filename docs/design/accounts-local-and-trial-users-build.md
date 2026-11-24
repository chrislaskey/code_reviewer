# Local users and trial users: the build plan

**Status: steps 1 to 5 built 2026-11-11.** The requirements settled in
`accounts-local-and-trial-users.md`, which keeps the scenarios and the
reasons, as steps with what each must do to be done. A ticked box is
built and tested; "As built" at the end lists where the code went
another way than a box says, and what is left. Checked by the tests on
SQLite and PostgreSQL, and the trial's pages in a headless browser; the
desktop and phone apps' pages were not clicked through.

Trials are off until the server is started with `TRIAL_USERS_MAX`
above 0; 100 is the number to start with, see step 5.

## The words

- **Full user**: signed up, with an account on the server.
- **Local user**: made by Skip on a desktop or phone, on that device
  only. Never syncs.
- **Trial user**: made by "Try it now" on the web, on the server,
  deleted 48 hours later unless they sign up. Web only.

A page or an app's code that must tell them apart reads one field,
`kind` on the current user: `full`, `local` or `trial`. Nothing else in
the app's code asks what kind a user is unless a requirement below
says so.

## Rules that hold in every step

- No merging of two accounts the server knows.
- A local user's records may move into an account by sign-up or by
  sign-in, since nothing a local user made exists anywhere else.
- Whether a device syncs is two separate checks: the user has an
  account on the server, and, once purchasing exists, their plan
  includes sync. Only the first is built here.
- Every step ends with `cargo fmt`, `clippy -D warnings`, the tests on
  SQLite and PostgreSQL, and `cargo audit` clean, and with the app
  working, so the work can stop between any two.
- Schema changes and new crates are named in their step and asked for
  before they are made.

## Step 1. Staying signed in on the web

The cookie today has no `Max-Age` and goes when the browser closes.

- [x] The web's sign-in and sign-up pages show "Keep me signed in on
      this browser", checked to start with. Whether it starts checked
      is a constant in core a fork changes.
- [x] The page sends the choice with whichever request finishes the
      sign-in: the code, the password, the new password, the reset.
- [x] Checked: the `token` cookie has a `Max-Age` of the `session`
      token's validity, sixty days. Unchecked: no `Max-Age`, as today.
- [x] The cookie's other attributes are unchanged. No change to
      `accounts_tokens`.
- [x] Not shown, and nothing set, when a device started the sign-in.
- [x] One function builds the cookie from the token and how long to
      keep it, so step 4 passes 48 hours to the same function.
- [x] Tests: the `Set-Cookie` header with and without the choice, on
      each finishing route.

Not here: renewing the sixty days on use; skipping the emailed code on
a remembered browser.

## Step 2. A community for every new user

- [x] A sign-up makes, in the transaction that makes the user, a
      community named `Personal` and the user's membership of it, as
      ordinary rows: they sync like any others.
- [x] Nothing marks the community as special. The person can rename
      or delete it like any community of theirs.
- [x] The code that makes "a user with their first community" takes
      the name, so steps 3 and 4 call it with the device's name and
      `Trial`.
- [x] An unconfirmed sign-up the purge removes takes its community
      and membership with it.
- [x] Tests: a new sign-up's current community is `Personal`; the
      purge of an unconfirmed sign-up leaves no community behind.

## Step 3. The local user

### 3a. Two changes to the sync, for everyone

- [x] **The created time comes from the event.** On a create the
      server sets `inserted_at` from the event's `made_at`, or from its
      own clock when `made_at` is later than that. One rule, in one
      place, for every synced table; no other time changes.
- [x] **A push goes in batches.** The standard push sends at most 500
      pending events a request, oldest first, and repeats until none
      are left. Each request's answers are applied before the next is
      sent, so a push cut short resumes where it stopped.
- [x] Tests: a post made offline keeps its time after the push; a
      `made_at` in the future gets the server's time; 1,200 pending
      events arrive in three requests and all apply; a failure on the
      second request leaves the first applied and the rest pending.

### 3b. Skip

- [x] The accounts page in the apps shows Skip while the device has no
      local user, and never after: one local user a device. Whether
      Skip exists at all is a constant in core a fork sets, on in the
      template.
- [x] Skip needs no network. It makes a user id, the account's folder
      and database, an `app_users` row named `local_<random>`, a
      community named after the device with the user's membership,
      and a `device_accounts` row with the email
      `local_<random>@example.com`, marked local. Then the app opens
      as that user, on the page a sign-in lands on.
- [x] The device's name is what the system answers, "iPhone" on an
      iPhone, and "This device" when it answers nothing.
- [x] `current_user` loads as for a full user, with `kind: local`.
- [x] The accounts page lists the local user as "On this device only"
      and switches to and from it like any account. It has no sign-out.
- [x] Remove from device on a local user says this is the only copy
      and cannot be undone.

### 3c. A local user's writes and sync

- [x] Every write works as for a full user, soft deletes included, but
      queues no event. The rows look like rows made offline and not
      yet pushed: `version` 0, `last_synced_at` null. One place in
      core decides this; no resource module asks the user's kind.
- [x] A sync for a local user makes no request and reports nothing.
- [x] The nav shows no pending count, no "Working offline" and no
      conflicts link for a local user.
- [x] The profile shows a banner: the account is only on this device,
      nothing is backed up, with Sign up and Sign in.
- [x] The links to the online-only apps stay. Each of those apps
      checks `kind` and shows that it needs an account, with the same
      Sign up and Sign in.
- [x] A local user has no picture upload.

### 3d. The move

- [x] Sign up and Sign in from the banner open the browser as today
      and come back with a device code and a session.
- [x] Before anything moves the app asks: "Move N items into
      <email>?" **Move** continues. **Keep them separate** adds the
      account beside the local user, which stays as it was.
- [x] The move is one core function, one transaction on the local
      user's file: write the account's user row; point every row of
      the local user at it; delete the local user's row and every
      soft-deleted row; queue a create for every row left, parents
      first, each with the row's `inserted_at` as its `made_at`; write
      `signed_in` with the token. The account's name and picture win.
- [x] The app then points the `device_accounts` row at the account's
      id and email, in the same folder, no longer local. Skip does not
      come back.
- [x] A crash between the two is healed at start: the file's
      `signed_in` user corrects the row.
- [x] From there the standard sync pushes the queue, files' bytes
      included, and the nav shows the count going down.
- [x] A create the server refuses is on the conflicts page like any
      refused change; the rest apply.
- [x] After a sign-up the person has `Personal` and the community
      named after the device. Nothing is renamed or removed for them.
- [x] Tests, on both databases with the in-process transport: a local
      user with posts, a file, a second community and a deleted post
      moves into a new account and into one that already has data;
      the server ends with every live row under the account with its
      original time, and the device matches the server; the deleted
      post is gone; Keep them separate changes nothing in the local
      file.

To ask before building: the `device_accounts` column that marks a
local user; `signed_in` holding a user without a token; the crate or
plugin for the device's name.

## Step 4. The trial user

Built with trials off in the template until step 5 is done.

- [x] Whether trials exist is a constant in core a fork sets.
- [x] "Try it now", a button on the public home page and the sign-in
      page, is a POST. It makes an `accounts_users` row with the email
      `trial_<random>@example.com` and `trial_expires_at` 24 hours on,
      an `app_users` row named `trial_<random>`, a community named
      `Trial` with the membership, and a `session` token, and sets the
      cookie of step 1 with a `Max-Age` of 48 hours.
- [x] With a live trial's cookie the button goes back into that trial;
      with a full user's session it goes to the application. It never
      makes a second user.
- [x] `current_user` has `kind: trial` and when the trial ends.
- [x] A banner on every page, with no way to close it, counts down the
      24 hours, then reads "Your trial ended. Sign up before <time> to
      keep what you made." Nothing else changes at 24 hours.
- [x] **Seen by nobody else.** A trial user cannot join a community;
      nobody can join one of theirs; their communities, memberships,
      posts and user row are in no other user's lists or pulls.
- [x] A trial user has no files and no picture, cannot sign in on a
      device, and the online-only apps show them what they show a
      local user.
- [x] **Signing up** from the banner is the sign-up pages with the
      trial's session. The code is mailed to the address typed; when
      it matches, that email and name replace the trial's on the same
      user, `confirmed_at` is set, `trial_expires_at` is cleared, and
      the cookie is set again as the checkbox says. No row moves. The
      community stays `Trial`.
- [x] A sign-up with an email that already has an account gets the
      quiet answer, and the trial is unchanged.
- [x] **The delete** is an hourly job beside the purge and the sweeps,
      run by the same loop and by `app_web jobs run`. It removes
      outright every user whose `trial_expires_at` is more than 24
      hours past, with their account, tokens, community and
      everything under them.
- [x] After the delete the cookie answers 401 and the person lands on
      the public home page.
- [ ] Ops marks trial users in its users list.
- [x] Tests: a trial's rows are absent from another user's lists and
      pull; join is refused both ways; a sign-up keeps the id and
      every row; the job leaves a 47-hour trial and removes a 49-hour
      one whole; a signed-up trial is never removed.

To ask before building: `trial_expires_at` on `accounts_users`; the
column on `accounts_tokens` that holds the address a code was sent to.

## As built: where the code differs

- **Whether "Keep me signed in" starts checked** is a constant in the
  pages, `svelte/src/accounts/keep_signed_in.js`, not in core: it is
  what a checkbox shows, and the server takes whatever the page sends.
- **The sign-up's community is made when the email is confirmed**, the
  first code that matches, not when the sign-up is posted. An
  unconfirmed sign-up can do nothing and so needs none, and the purge
  has no community to clean up after it.
- **Skip is offered while the device has no account of any kind**, not
  only no local user: it is the way in for someone new. So it is gone
  once anyone is on the device, and after a move; it comes back if
  every account is removed.
- **A move is offered only into an account that is new to the device.**
  An account that already has a folder on the device is signed in
  beside the local user as before; moving into it would mean joining
  two folders. Left for later.
- **The session of a sign-in whose move is offered is held in memory**
  until the person answers. Quitting the app at that question means
  signing in again.
- **A push keeps to what waited when it began.** A change made while
  the push runs waits for the next sync, as it did before batches.
- **Whether trials exist is `TRIAL_USERS_MAX`**, the cap agreed for
  abuse, read when the server starts and 0 unless set, in place of a
  constant: one switch, and it can be turned off without a build.
- **A trial user has no Sign out**, as a local user has none: the
  cookie is the only way back in.
- **After the delete** the person lands on the sign-in page, which the
  router sends anyone with no session to, not the public home page.
- **"Try it now" on the public home page** is a link to
  `/accounts/trial`, whose button makes the trial, so opening a link
  makes nothing. The link shows once `GET /api/accounts/trial` says a
  trial can start.
- **A trial user's made-up email gets no mail**: a sign-in with it is
  answered like any other and nothing is sent.
- **When a trial signs up, their communities are written as changed**,
  so devices that pulled while they were hidden get them.

Not built:

- **Ops marking trial users**: ops has no users list yet. The current
  user's `kind` and `accounts_users.trial_expires_at` are what it
  will read.

## Step 5. Abuse of trials

Agreed and built 2026-11-11; the reasons are in To decide 13.

- [x] **The cap stays off unless set.** `TRIAL_USERS_MAX` unset is 0 on
      a deployed server. `bin/dev` sets it to 100, and the README
      suggests 100 as the number to start with.
- [x] **Ten trials an hour per client IP**, a bucket of its own on
      `POST /api/accounts/trial`, on top of the accounts routes' 30
      requests a minute.
- [x] **The client IP behind a proxy is the last entry of
      `X-Forwarded-For`**, the one the proxy wrote, not the first,
      which the client can write. For every per-IP limit.
- [x] **Maximum lengths, for everyone**: a post's body 10,000
      characters, a community's name 80. Checked in core, so a device
      refuses what the server would.
- [x] **500 writes in a trial's life.** The server already records
      every write as a row in `app_events` with the user's id, in the
      write's transaction. Where it records one, core counts the
      user's rows when the user is a trial, and at 500 refuses the
      write: "This trial has reached its limit. Sign up to keep
      going." Nothing is recorded for a refused write. No resource
      module asks the user's kind, and a push from a trial's cookie
      is counted like any write. Signing up ends the limit.
- [x] **A trial user lists only their own communities.**
- [x] **The server's log says when a trial is refused** because the
      cap is reached.
- [x] **A push fits the request the server takes.** Before this a
      push was 500 events whatever their size, the server took 2 MB,
      and a push over that was refused whole and sent again unchanged
      at every sync, so the device never pushed again. One constant
      in core, 5 MB, is the most a push request may be. The server
      sets that as the body limit of `/api/sync/push` only; the other
      routes keep 2 MB. A device fills a batch until 500 events or
      four fifths of the constant, by the events' serialized size,
      and always sends at least one. The README says a reverse proxy
      in front must allow 5 MB too.
- [x] Tests: the eleventh trial in an hour from one address is 429
      and another address is not; a forged first `X-Forwarded-For`
      entry does not change the key; a body of 10,001 characters is
      refused on both databases; a trial's 501st write is refused and
      leaves no row, and the same user after signing up writes again;
      a trial's list has no community of anyone else; 500 pending
      events with bodies of the maximum length go in several requests
      and all apply; a push over 5 MB is 413.
- [x] `docs/suggestions/accounts.md`: a CAPTCHA in front of "Try it
      now"; ops showing the count of trials against the cap; a rate
      limit per user on the application's routes.

Accepted, nothing built: one person taking trial after trial; ten an
hour from one address being enough to keep a cap of 100 full, which
makes trials unavailable and never sign-up; two requests at once
passing the cap by a few.

As built, where it differs from a box:

- **The 500 are counted with no index.** `app_events` has none on
  `user_id`, so each write of a trial user reads the whole table;
  a full user's write only reads their account's row. An index is a
  migration, to ask for.
- **A write over the 500 is an error, not a rejected answer.** The
  transaction ends and the request is answered 403, a whole push
  too, since only a browser holds a trial's session and no device
  waits on the answer.
- **Only their own communities, in every read**: the list, one
  community by its id, and a pull.
- **No new line in the log for a full cap.** The refusal is a 503,
  and `ApiError` already writes every 5xx to the log as an error,
  with "Trials are full right now".
- **A device's name longer than 80 characters is cut to 80** for the
  local user's community, so Skip cannot fail on it.
- **The lengths are counted in characters**, not bytes, as a user's
  name is.

## Left out

In `docs/suggestions/accounts.md` when this is built: exporting a local
user's data without signing up; going back from a full user to a local
one; files for trial users; extending a trial; a CAPTCHA, unless step 5
takes it.
