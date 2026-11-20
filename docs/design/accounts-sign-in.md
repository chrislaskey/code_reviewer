# Signing in: the emailed code, and devices through the browser

**Status: built, 2026-11-08, steps 1 to 6 of the build order; the
later returns are not.** Since `multi-app.md` the email and
`confirmed_at` are on `accounts_users`, a server-only row with the
user's id, and a sign-up takes a name, the only thing of a person that
`app_users` syncs besides their picture; read `app_users` below as
that pair. The code is `core/src/accounts.rs`,
`core/src/tokens.rs`, `core/src/mailer.rs` and `core/src/mails.rs`,
`axum/src/accounts.rs`, `axum/src/mailer.rs` and
`axum/src/rate_limits.rs`, `tauri/src/accounts.rs` with the
`device_sign_in` table in `tauri/src/device_accounts.rs`, and the pages
under `svelte/src/accounts/`. Every item under "To decide" is
**Agreed**; each carries the recommendation and what else was weighed; the sections after it describe the design as recommended, so
it can be read whole, and change where a decision goes the other way.

## Since 2026-11-11: the code first, a password optional

The emailed code is now what signs a person in, and a password is an
optional second factor asked for after it. This section is the flow as
built; where "To decide" below speaks of a password checked before the
code, or made at sign-up (items 4, 5 and 22), read it as moved here.

- **Sign-up** takes a name and an email, no password. **Sign-in** takes
  an email. Both mail a code and answer the pending token. A sign-in
  with an email that has no account answers the same way, and that
  address is mailed that it has none, as a sign-up with a taken email
  always was.
- **The code** proves the email, then the server looks at the account's
  second factors, `accounts::second_factors`, a list with one possible
  entry, `password`:
  - the account has a password: the code answers
    `{ next: "password", pending, can_reset_password }`, and the password finishes the
    sign-in, `sign_in_with_password`;
  - the account has none and this code is its first, the one that
    confirms the email: the code answers
    `{ next: "new_password", pending }`, and the page offers "Create a
    password" with Skip, `sign_in_with_new_password`, a password or
    none;
  - otherwise the code finishes the sign-in as before.
- **The step's pending token** is a new `sign_in_password` row in
  `accounts_tokens`, ten minutes, made when the code matched. A wrong
  password counts against it as a wrong code does, and the fifth
  deletes it, so guessing a password costs a new emailed code every
  five tries. Nobody is signed in until the step is taken: closing the
  page at "Create a password" leaves a confirmed account with no
  password and no session, and the next sign-in is the code alone.
- **A device's sign-in** is the same pages with its challenge; whichever
  step finishes answers the device code.
- **Later**, signed in, `/accounts/password`
  (`AccountsPasswordEditPage.svelte`) makes the first password or
  changes it, the current one required when there is one,
  `accounts::password_update`. In the apps the page asks the server
  through `accounts_second_factors` and `accounts_password_update`, so
  it needs a connection, and it is the one place a password passes
  through an app.

- **Staying signed in**: the web's Sign in and Sign up pages have
  "Keep me signed in on this browser", checked to start with
  (`svelte/src/accounts/keep_signed_in.js`). The page sends it as
  `keep_signed_in` with whichever request finishes the sign-in, and
  the cookie then gets a `Max-Age` of the `session` token's sixty
  days; without it the cookie goes when the browser closes, as before.
  Not offered for a device's sign-in, which sets no cookie.

- **Before signing up**: a person may use the app as a local user on a
  device or a trial user on the web, see
  `accounts-local-and-trial-users.md`. `POST /api/accounts/trial`
  makes a trial user, and a sign-up from a trial user's browser is
  `accounts::sign_up_from_trial`: the code goes to the address typed,
  kept on the pending row as `sent_to`, and the same user takes it
  when the code matches.

- **A forgotten password** is the app's choice,
  `accounts::FORGOTTEN_PASSWORD`, a constant to change in a fork:
  - `Reset`, the default: the password step shows "Forgot your
    password?", and the person, whose code just matched, makes a new
    one in its place and is signed in, `sign_in_with_reset_password`.
    Their email is told "Your password was changed". The password then
    guards only against someone who cannot read the inbox, so it is no
    true second factor; that is the price of nobody being locked out.
  - `LockedOut`: no link, the route answers 403, and an account stays
    closed without its password. A fork that chooses this needs another
    way back, recovery codes or staff clearing it, see
    `docs/suggestions/accounts.md`.

  The code's answer carries `can_reset_password`, so the page shows the
  link only when it would work.

## Since 2026-11-20: the desktop and the iPhone return the code themselves

On a desktop the person no longer pastes the code.
`accounts_sign_in_start` starts a listener on `127.0.0.1`, on a port
the system picks, through `tauri-plugin-oauth`, and adds
`app_port=<port>` to the sign-in page's query; the sign-in and sign-up
links keep it, as they keep the challenge. When the page has the
device code it navigates to `http://127.0.0.1:<port>/?code=…`, built by
`accountsAppReturnUrl`, which takes only a port, so the page can send
a code nowhere but this computer. The listener answers its first
request with a redirect to `/accounts/back-to-app` on the web and
hands the URL to `tauri/src/accounts_loopback.rs`, which finishes the
sign-in as a pasted code does, brings the window to the front, and
emits `accounts-sign-in-returned` for the accounts page, `{result:
"finished", finished}` or `{result: "failed", error}`. The page still
shows the code under "Returning you to the app…", and the dialog keeps
its field, so pasting stays the fallback: on a phone, when the
listener cannot start, or when the app was quit while the browser was
open. A new start, a cancel or a pasted finish stops the listener; a
stray request to the port ends it with a code that is useless without
the verifier.

**On an iPhone, the sign-in sheet**, `ASWebAuthenticationSession`,
in place of universal links. iOS does not open the app from a universal
link on the domain the person is already browsing, nor from a redirect
they did not tap, and the end of a sign-in on the server's own pages is
both. `accounts_sign_in_start` opens the sign-in page in the sheet over
the app, `tauri/src/accounts_sheet.rs` through our own plugin,
`tauri/plugins/web-auth-session` (the published ones either drop the
session, so the sheet can close at once, or are called only from
JavaScript). The query carries `app_scheme`, the app's bundle
identifier, and the page, done, goes to
`com.starterpack.app://accounts/sign-in?code=…`; `accountsAppReturnUrl`
takes that one scheme only, a constant in `routes.js` to change with
the identifier in a fork. The sheet closes and hands the URL to the
session that opened it alone, so another app claiming the scheme gets
nothing, and PKCE still guards the code; the app finishes as with the
loopback, with the same event, `accounts-sign-in-returned`. The sheet
is private, `prefersEphemeralWebBrowserSession`, sharing no cookies
with Safari, which a device sign-in never sets anyway, so iOS asks no
"wants to use … to sign in" first. The person closing the sheet leaves
the sign-in waiting with the dialog's field, so a code copied in the
sheet can still be pasted; the dialog's Cancel ends it. The custom
scheme needs no entry in `Info.plist` for the sheet; the `https`
callback, iOS 17.4 and later, would need the associated domain, and the
app supports iOS 15. Android still pastes. Universal links stay for
links in emails, later.

## Before that

A sign-up or a sign-in was one request with an email and a
password, on the web from the sign-in page and in the apps from the
same page through a Tauri command, which sends the password to the
server over the sync transport (`core/src/accounts.rs`,
`docs/design/device-accounts.md`). The server never stores a password
anywhere but its own credentials table, a device never pushes a user
row, and there is no offline sign-in. What changes here:

1. **Every sign-up and sign-in ends with a code emailed to the person**,
   which they type in to finish. The email is sent by a mailer: logged
   in local dev, sent through Resend in production.
2. **The apps stop asking for a password.** A device signs in in the
   system browser, on the web's own sign-in pages, and the browser
   hands the app a one-time code, which the app exchanges for its API
   token. The app keeps one flow forever: open the page, receive a
   code, exchange it. Every later way of signing in, a password reset,
   a magic link, Google or Apple, is a change to the web pages and the
   server, and reaches every installed app the day it deploys.
3. **The foundations for OAuth**: the device flow does not care how the
   browser session proved who the person is, and every way of proving
   it ends in one private function that either signs the browser in or
   makes a device code.

Not here: OAuth itself, password reset, magic links, changing the
email or password, rate limiting in front of the server, see
`docs/suggestions/accounts.md`.

## To decide

1. **Reuse `accounts_tokens` for every secret, with a `context` column**,
   as `mix phx.gen.auth` does, rather than a table per kind of secret.
   Three contexts: `api` for today's rows, the apps' bearer tokens and
   the browser's cookie; `sign_in_code` for a sign-in that passed the
   password and waits for its emailed code; `device_code` for a
   finished browser sign-in waiting for the device to collect it. Each
   context has its own validity and the purge deletes by it. The
   `sign_in_code` rows need two more columns, `code_hash` and
   `attempts`, see "The tokens table". The alternative, a
   `users_sign_ins` table for the pending ones, keeps `accounts_tokens` as
   it is but is a second pattern for the same thing. **Agreed: one
   table.** Phoenix's `sent_to` column is left out until an email can be
   changed, which is what it is for.

2. **The code is six digits**, valid for ten minutes, five wrong
   attempts and the pending sign-in is deleted, the person starts over.
   The alternatives, eight letters and digits with no attempt cap, or a
   link to click, are later options; a link to click is the magic-link
   flow, which can reuse all of this. The ten minutes are the server's
   clock from the row's `inserted_at`, checked at the lookup and used
   by the purge; a new code from the resend button restarts them on
   the same row. **Agreed: six digits, ten minutes, five attempts.**

3. **The code is asked for on every sign-in, on the web too.** The
   cookie lasts the browser session, so a web user types a code each
   time they open a new browser. A device signs in once per account and
   keeps its token for a year, so there the code is rare. A "remember
   this browser for thirty days" is a later option, Phoenix's
   remember-me, and is left out. **Agreed: every time; remember-me
   deferred.**

4. **A sign-up creates the user at once, unconfirmed**, with
   `confirmed_at` on `app_users` as Phoenix has it, set when the code
   matches. An unconfirmed user has no token and can do nothing. A
   second sign-up with that email, before it is confirmed, replaces the
   password and sends a new code, so whoever reads the inbox gets the
   account and a stranger cannot block an email they do not own. A
   sign-in to an unconfirmed email with the right password sends the
   code and confirms on the match. The purge deletes unconfirmed users
   older than a day, with their credentials. `app_users` is synced, so
   the column rides along to devices, harmless and unused there; the
   alternative is a server-only `users_confirmations` table, more
   pieces for the same fact. **Agreed: `confirmed_at` on `app_users`**,
   a migration of a synced table and the `User` struct.

5. **A sign-up with a confirmed email answers as a new one does**, "we
   sent a code to that address", and the server emails the account's
   owner instead: "You already have an account here; sign in with your
   password." The page never tells whether an email has an account;
   the sign-in never did, since a wrong password and an unknown email
   are one answer. A person who forgot they had an account lands on
   the code field with no code coming, and the email tells them what
   to do; that dead end is the cost of not telling a stranger. The two
   answers must take the same time, so both send an email. To be
   exact about Phoenix: its sign-up form does say "has already been
   taken"; it is the reset-password and magic-link flows that answer
   "if your email is in our system, you will receive instructions".
   This goes a step further than Phoenix. **Agreed: the quiet
   answer.**

6. **The device learns the code by the person copying it**, first. The
   web page, when it finishes a sign-in started by a device, shows the
   code with a Copy button and "Paste it into the app", and the app's
   dialog has the field. This works on every platform, in dev and
   installed, with no scheme registered anywhere, and it stays as the
   fallback once the automatic returns exist. The automatic returns
   come as their own steps after, in this order. **Desktop, a loopback
   redirect**: the app listens on `127.0.0.1` on a port of its own
   before opening the browser, passes that address in the URL, and
   the page redirects there with the code; the listener answers a
   one-line "you can return to the app" and finishes. It works in dev
   and installed on macOS, Windows and Linux, needs no scheme and no
   second process, and is what RFC 8252 recommends for desktop; the
   server accepts only loopback addresses as that redirect, so it is
   never an open redirector. A desktop deep link was weighed and set
   aside: on macOS it only works for a bundled app installed in
   /Applications, never under `tauri dev`, and on Windows and Linux it
   starts a second process unless the single-instance plugin is added.
   **Mobile, universal links**, deferred: an `https` link on the app's
   own domain that only this app may open, verified by
   `.well-known/apple-app-site-association` served by the web server
   with the Apple team id, and `assetlinks.json` with the signing
   certificate's fingerprint on Android, through
   `tauri-plugin-deep-link`, which takes the URL whether the app is
   running or has to be launched. Going straight to universal links,
   not a custom scheme first, is the choice; the custom scheme can be
   claimed by any app, and the verifier is the only thing that keeps
   a taken code useless. In local dev they are possible but fiddly:
   iOS needs the entitlement in `?mode=developer`, Associated Domains
   Development switched on under Settings > Developer, the web server
   on HTTPS under a local CA the device trusts, and reports differ on
   whether the simulator fetches the file at all, so a physical phone
   is the reliable check; Android 12 and later can be told to trust
   the domain with `adb shell pm set-app-links`. Worked out when that
   step starts. **Agreed: copy-and-paste in this build, the desktop
   loopback next, universal links on mobile after, deferred.**

7. **The exchange is bound to the app that started it**, with a secret
   the app keeps and a hash of it in the URL, which is PKCE from OAuth:
   the app makes a random verifier, sends its SHA-256 as `challenge`
   in the sign-in URL, and sends the verifier itself with the code.
   The server keeps `hash(code + challenge)` as the device row's
   `token_hash`, so the row cannot be found without both and no column
   is added; a code taken off a deep link by another app, or read over
   a shoulder, is useless on its own. One hash each side; recommended,
   from the start, so the exchange never changes shape when the deep
   links arrive. **Agreed in principle, to solve early**; see "The
   exchange, step by step" for what it does.

8. **Where the app keeps the verifier while the browser is open**:
   in memory, as Tauri state, or in `device.db` in a one-row table,
   `device_sign_in`, holding the verifier, the email the sign-in was
   started with, and when it started. In memory it lives as long as
   the app's process. On a desktop the process stays while the person
   is in the browser, unless they quit the app. On a phone the app is
   in the background the whole time: iOS suspends it and ends it under
   memory pressure, Android ends backgrounded apps readily, and some
   makers' battery savers do so within minutes. When the person comes
   back, by the deep link or by hand with the code copied, a relaunched
   app has no verifier and no waiting page: the code is useless and
   they start over. With the row in `device.db` the app sees at launch
   that a sign-in started less than ten minutes ago, shows the waiting
   page again with the field, and finishes with the pasted or
   delivered code. The row goes when the sign-in finishes, is
   cancelled, or is older than ten minutes at a launch. The desktop
   loopback, running in the app's process, needs none of this, and a
   desktop that was quit shows the dialog again from the row. Starting a
   sign-in while one waits replaces it. The verifier sits on disk for
   minutes, useless without the code, in a file that already lists
   the accounts; the account files hold year-long tokens. The table
   is one app-level migration under `tauri/migrations/device` and some
   forty lines. **Agreed: `device.db` from the start**, since the
   copy-and-paste path on a phone needs it as much as the deep link
   will.

9. **A device sign-in sets no cookie in the browser.** When the code
   request carries a `challenge`, the server answers the device code
   instead of the session, and the browser is not signed in. Nothing
   is left behind on a shared computer, and a browser already signed
   in as someone else does not matter: the page shows the form either
   way. "Continue as the person signed in here" is a later nicety.
   **Agreed.**

10. **The mailer is an enum in core**, `mailer::Mailer`, with one
    `send(Mail)`, where `Mail` is to, subject and text, composed in
    core by `mails`, see "The mailer". Three variants: `Log`, which
    writes the whole email at `info` for dev; `Memory`, which keeps
    the emails in a list for the tests of both crates to read the
    code from, and for a dev mailbox page one day; and `Resend`
    behind a core feature `mail-resend`, one POST to
    `https://api.resend.com/emails` with the API key as a bearer
    token. An enum rather than a trait because the server picks one
    at start, from `MAILER=log|resend`, `log` when unset, with
    `RESEND_API_KEY` and `MAIL_FROM` for Resend, and the set is
    closed; `Transport` is a trait because each build compiles in one
    and tests another. Library: Resend has an official crate,
    `resend-rs`, on `reqwest`, which the workspace already has; the
    POST is ten lines with `reqwest` directly, the crate brings the
    rest of the API we do not use. The rules say to ask before writing
    our own: **agreed, `reqwest` directly**, since the whole use is one
    request, and `resend-rs` if more of Resend is ever needed. SMTP
    through `lettre` is the provider-neutral alternative, left out.

11. **The email is sent after the transaction commits**, not inside it,
    and a send that fails answers the request with an error; the
    pending row expires on its own. Sending runs in the request, a
    fraction of a second with Resend; a queue is a later matter.
    **Agreed.**

12. **Where the app sends the browser: `BROWSER_SIGN_IN_URL`.** The
    app opens the web's sign-in page in the browser, which in
    production is on the server's own origin, `SYNC_SERVER_URL`. In dev
    the pages come from the Svelte dev server, 1420 with its proxy to
    the API, not from 4000. One more variable names the page itself,
    `BROWSER_SIGN_IN_URL=http://localhost:1420/accounts/sign-in`, read
    as `SYNC_SERVER_URL` is, falling back to `SYNC_SERVER_URL` plus
    `/accounts/sign-in` when unset; `bin/dev` sets it for the desktop
    and mobile targets. The name says what the app does with it, which
    `PAGES_URL` did not. **Agreed.**

13. **The apps' sign-up and sign-in pages go.** `AccountsSignInPage`
    and `AccountsSignUpPage` stay for the web only; in the app the
    router sends their paths to `/accounts`, where Add account and
    Sign in open the browser and show the waiting dialog, see "The
    app". The `Transport` trait loses `sign_up` and `sign_in` and
    gains the device code exchange, so no code in an app build sends
    a password anywhere; the server's password routes stay reachable
    by any HTTP client, as any route is, and serve the web pages. In
    core, `sign_up_through`, `sign_in_through`, `sign_up_on_device`
    and `sign_in_on_device` go. The tests that signed a device up
    through `InProcess` get a helper that signs up on the server with
    the `Memory` mailer, finishes with the code, and hands the device
    the session. The ignored two-device HTTP test cannot read a code
    from a live server's log; it opens the server's database itself,
    from `TEST_SERVER_DATABASE_URL`, signs up there, and syncs over
    HTTP as before. **Agreed.**

14. **A "Send a new code" button is part of the first version**, as
    its own build step after the flows work, to keep the first step
    small: `POST /api/accounts/code/resend` with the pending token
    makes a new code on the same row, resets the attempts, and mails
    it, at most once a minute per pending sign-in, see "Rate limits".
    Until that step, a person who did not
    get the email signs in again, which sends a new code and deletes
    the old pending row. **Agreed.**

15. **The device code is 16 random bytes as hex**, 32 characters,
    valid five minutes, used once. Short enough to paste. **Agreed as
    the starting point.**

16. **Rate limits with the `governor` crate**, the nearest thing to
    Elixir's `hammer`: a limiter keyed by any value, one bucket per
    key, in memory, see "Rate limits". Three keys in this build: the
    emails sent per address, the resends per pending sign-in, and the
    requests per client IP on the accounts routes, the last through
    `tower_governor`, its axum middleware. The wrong-code attempts stay
    a column, since they must survive a restart. Unlike `hammer`,
    `governor` has no Redis backend: the buckets are this process's,
    which is right for one server and would need a shared store or a
    limit at the proxy for several. Starting numbers: five emails an
    hour per address, one resend a minute per pending sign-in, thirty
    requests a minute per IP on the accounts routes, to tune.
    **Agreed, starting there.**

17. **The waiting state is a dialog on the accounts page**, not a page
    of its own, see "The app" for the two shapes. **Agreed to start;
    a page later if a full screen reads better on a phone.**

18. **Emails are lowercased** when a user is made and when one is
    looked up, so `Alice@example.com` and `alice@example.com` are one
    account. Today an email is stored as typed, trimmed, and matched
    exactly, so they are two; with a code mailed to the address, the
    quiet sign-up answer and a limit keyed by the address, the two
    spellings must mean one person. Phoenix gets this from
    PostgreSQL's `citext`; lowercasing on the way in works on both
    databases. The dev data is reset. **Agreed.**

19. **Two kinds of API token now that the column exists**: `session`
    for the browser's cookie, sixty days as Phoenix's sessions, and
    `api` for the apps, a year as today, instead of one `api` for
    both. A web sign-in makes a token per browser session and the
    cookie forgets it when the browser closes, so year-long rows pile
    up until the purge; sixty days shortens that, and the split is
    what a remember-me needs later. The suggestions file lists this
    as deferred; the column makes it a line each in `finish_in` and
    `validity`. **Agreed, now.**

20. **The purge deletes unconfirmed users outright**, row, credentials
    and pending sign-in, with no `deleted_at` and no event. To be
    plain about where these rows are: a user is only ever created on
    the server, by a sign-up on the web page, whether that page was
    opened in a laptop's browser or by a phone's app in its browser;
    no device ever creates a user. An unconfirmed user is a server row
    for a sign-up whose code was never entered, a day or more ago,
    with no token, no data and no membership, that no device has ever
    pulled. Deleting it outright is a server-side housekeeping of a
    row nobody saw; the soft delete and the ninety days are for rows a
    device may hold. A confirmed user is never touched by this, and
    an unconfirmed one could never use the app, since no token is made
    before the code matches. **Agreed.** The day is the purge's daily
    run, not a grace period: a person who comes back sooner signs up
    again, which resends the code.

21. **The pending token lives in the page's memory** on the web, not
    in the browser's storage: a reload of the code page means signing
    in again. Simplest, and nothing secret stays in the browser.
    **Agreed.**

22. **Both sign-up answers take the same time**: the one for a new
    email and the one for a taken email each hash a password and send
    an email, so neither the clock nor the response tells them apart.
    Follows from 5; noted so the build does it. **Agreed.**

23. **The order of the return paths** is settled in 6: copy-and-paste,
    the desktop loopback, universal links on mobile. **Agreed.**

## The flows

**Sign-up on the web.** The page posts the name and the email. The
server makes the account, unconfirmed, and the user's row, deletes any
earlier pending sign-in of theirs, makes a `sign_in_code` row with a
random pending token and the hash of a six-digit code, commits, mails
the code, and answers the pending token. For an email that already
has a confirmed account it answers the same, and mails that account's
owner that they already have one, see "To decide" 5. The page shows "We sent a code
to alice@example.com" and the code field. The page posts the pending
token and the code. The server finds the row by the token's hash,
refuses one expired, counts the attempt, compares the code's hash,
deletes the row, sets `confirmed_at`, and answers the next step,
`new_password`, with a new pending token. The page shows "Create a
password" and Skip, and posts the pending token with the password, or
none; the server keeps the password if there is one, makes a `session`
token, sets the cookie and answers the session. Done, the person sees
the posts.

**Sign-in on the web.** The page posts the email, and the code goes
the same way. Once it matches, an account with a password is answered
the next step, `password`, and the page asks for it; an account
without one is signed in at once. The email is lowercased before the
lookup, as it was when the account was made.

**Sign-in from a device.** On the accounts page, Add account or Sign in
on a signed-out account. The app makes the verifier and its challenge,
opens `BROWSER_SIGN_IN_URL?challenge=…&email=…` in the system
browser, and shows "Finish signing in in your browser", the field for
the code, and Cancel. The browser runs the web sign-in or sign-up above
with one difference: the code request carries the challenge, so the
server answers a device code and sets no cookie: a `device_code` row
with `hash(code + challenge)`, five minutes. The page shows the code
with Copy and says to paste it into the app; on a desktop it
redirects to the app's loopback address with the code, see "Since
2026-11-20". The person
pastes it; the app posts the code and the verifier; the server finds
the row by `hash(code + sha256(verifier))`, deletes it, makes an `api`
token and answers the session; the app hands the session to
`keep_signed_in`, which opens or makes the user's file as today, and
syncs. The sign-up link on the page keeps the challenge, so a new
account from a device is the same.

**Sign-out and a 401** are unchanged, see `device-accounts.md`.

**A sign-in a person abandons** leaves a `sign_in_code` row the purge
deletes after it expires, or the next attempt replaces, and, for a
sign-up, an unconfirmed user the purge deletes after a day.

## The exchange, step by step

The device code travels where the app does not control it: shown on a
web page, copied through the clipboard, later carried by a deep link
that another app on the phone could have registered for. Anyone who
sees the code must not be able to turn it into a token. PKCE, the
OAuth name for the following, makes the code useless without a secret
that never leaves the app:

1. Before opening the browser the app makes 32 random bytes, the
   **verifier**, and keeps it. It hashes the verifier with SHA-256; the
   hash is the **challenge**.
2. The app opens the sign-in page with the challenge in the URL. The
   challenge may be seen by anyone; a hash cannot be reversed.
3. The person signs in. The server makes the device code and keeps the
   row under `hash(code + challenge)`.
4. The app sends the code and the verifier. The server hashes the
   verifier to get the challenge back, computes `hash(code + challenge)`,
   finds the row, deletes it and answers the token.
5. Someone with the code alone, and even the challenge from the URL,
   cannot compute the lookup: they need the verifier, and only the app
   that started the sign-in has it.

Cost: one random value and two hashes in the app, one hash on the
server, no column.

## The tokens table

`accounts_tokens` gains three columns:

| column | for |
| --- | --- |
| `context TEXT NOT NULL DEFAULT 'api'` | `api`, `session`, `sign_in_code` or `device_code`; today's rows become `api` |
| `code_hash TEXT` | `sign_in_code` only: SHA-256 of the six digits |
| `attempts INTEGER NOT NULL DEFAULT 0` | `sign_in_code` only: wrong codes so far |

`token_hash` stays the lookup for every context: the hash of the API
token, of the pending token, or of the device code joined with the
challenge. Validity by context: `api` 365 days as today, `session`
sixty, `sign_in_code` ten minutes, `device_code` five, in
`tokens::validity(context)`;
`delete_expired` deletes each context by its own age, and the purge
reports them as one count as today. A row is found by its hash and its
context together, so a pending token never passes as an API token.

`app_users` gains `confirmed_at TEXT`, null until the first code
matches, synced like the row's other columns.

Both are migrations, SQLite and PostgreSQL, and are asked for here.

## The mailer

`core/src/mailer.rs`: `Mail { to, subject, text }`, `MailError`, and
`enum Mailer { Log, Memory(Arc<Mutex<Vec<Mail>>>), Resend(Resend) }`
with `async fn send(&self, mail: Mail) -> Result<(), MailError>`.
`Log` writes the email in full at `info` through `tracing`, every
line, so a dev reads the code in the server's terminal. `Memory`
appends to its list; a test takes the last email to the address and
reads the six digits out of it. `Resend { client, api_key, from }`,
behind `mail-resend`, posts `from`, `to`, `subject`, `text` with
`Authorization: Bearer`, and answers `MailError::Refused` with the
status and body on anything but 200, `MailError::Transport` when the
request fails; the key is a `SecretString` from the `secrecy` crate,
read from the environment at start, so it never prints and is only
exposed at the one line that sets the header.

`core/src/mails.rs` composes every email the app sends, text only:
`mails::sign_in_code(to, code)`, subject "Your Starter Pack code",
"Your code is 123456. It expires in ten minutes. If you did not ask
for it, ignore this email."; `mails::already_signed_up(to)`, subject
"You already have an account", "Someone, likely you, tried to sign up
with this email. You already have an account: sign in with your
password." The name of the app in the subject comes from one
constant, to change in a fork. HTML, templates and a sender name are
later.

The server opens the mailer at start beside the database,
`axum/src/mailer.rs`: `MAILER` unset or `log` gives `Log`, `resend`
gives `Resend` from `RESEND_API_KEY` and `MAIL_FROM`, and the server
refuses to start when either is missing. The router's state becomes
a struct of the deployment and the mailer, with `FromRef` for each,
so the `CurrentUser` extractor keeps taking the deployment. A send
that fails answers 503 "Could not send the email; try again". `bin/dev`
and the README name the variables; `.env` files stay uncommitted.
What stops a stranger from making the server email any address over
and over is the per-address limit, see "Rate limits".

## The server's API

| route | body | answers |
| --- | --- | --- |
| `POST /api/accounts/sign-up` | `email`, `name` | 202 `{ pending }` whether the email is new or not, 422 bad email or name |
| `POST /api/accounts/sign-in` | `email` | 202 `{ pending }` whether the email has an account or not, 422 bad email |
| `POST /api/accounts/code` | `pending`, `code`, `challenge?` | 200 session with the cookie, or with `challenge` 200 `{ code }` and no cookie; or 200 `{ next, pending }`, `next` being `password` or `new_password`; 401 wrong, expired, or too many |
| `POST /api/accounts/sign-in/password` | `pending`, `password`, `challenge?` | 200 as a finished code; 422 wrong password; 401 expired or too many |
| `POST /api/accounts/sign-in/reset-password` | `pending`, `password`, `challenge?` | 200 as a finished code, the old password replaced and its owner emailed; 403 when the app locks out; 422 a password too short or long; 401 expired |
| `POST /api/accounts/sign-in/new-password` | `pending`, `password` or null to skip, `challenge?` | 200 as a finished code; 422 a password too short or long; 401 expired |
| `GET /api/accounts/second-factors` | | 200 `["password"]` or `[]`, of the signed-in user |
| `PUT /api/accounts/password` | `current` or null, `password` | 204; 422 wrong current password, or one too short or long |
| `POST /api/accounts/code/resend` | `pending` | 202, a new code mailed; 429 within a minute of the last, see "Rate limits" |
| `POST /api/accounts/device-token` | `code`, `verifier` | 200 session, no cookie; 401 |
| `DELETE /api/accounts/sign-out` | | 204, unchanged |

In core, `accounts`, server side: `sign_up`, `sign_in`, both taking the
mailer and answering `CodeSent { pending }`; `sign_in_with_code`
answering a `Session`; `sign_in_with_code_for_device` answering a
`DeviceCode`; `sign_in_with_device_code` answering a `Session`;
`sign_out` as today. The two code functions share one private
`match_code_in(tx, pending, code) -> User`, and every function that has
a proven user ends in one private `finish_in(tx, user, Finish)` where
`Finish` is `Session` or `Device { challenge }`. That last function is
the OAuth foundation: a provider callback, later, is one more public
function that proves a user its own way and calls it.

Device side: `sign_in_with_device_code_through(transport, code,
verifier) -> Session` replaces `sign_in_through` and `sign_up_through`;
`keep_signed_in`, `sign_out_on_device` and `forget_signed_in` stay.
`Transport` gets `sign_in_with_device_code` in place of `sign_up` and
`sign_in`; `InProcess` calls the core function, `Http` posts the route.
The ignored two-device HTTP test and the core tests that sign devices up
through the transport sign up on the server directly and take the
session from there instead.

## Rate limits

`axum/src/rate_limits.rs`, in the server's state beside the deployment
and the mailer: `RateLimits` holding keyed limiters from `governor`,
`emails` keyed by the lowercased address and `resends` keyed by the
pending token's hash, each a `Quota`, checked with `check_key` by the
accounts handlers before they call core; a refusal is 429 "Too many
attempts; try again in a few minutes". The buckets live in memory and
are forgotten on a restart, which is fine for these; a task once an
hour calls `retain_recent`, so the map does not grow with every
address ever tried. The per-IP limit is `tower_governor`'s layer on
the accounts routes only, keyed by the peer address, which needs the
server started with `into_make_service_with_connect_info`; behind a
reverse proxy the key is the last entry of `X-Forwarded-For`, the
one the proxy wrote: the entries before it are whatever the client
sent, so the first, which `SmartIpKeyExtractor` takes, let a client
name a new address with every request. And only when the proxy is the
only way in, since the header is otherwise a lie anyone can send. That
switch is a variable, `TRUST_PROXY_HEADERS`, off by default. "Try it
now" has a bucket of its own per client IP, ten an hour, checked in
its handler, see `accounts-local-and-trial-users.md`. Core knows nothing of
any of it; the same `RateLimits` can later take a key for anything the
server wants to meter, sync pushes per token or posts per user.

## The app

`tauri/src/accounts.rs`: `accounts_sign_in_start(email?)` makes the
verifier, writes the `device_sign_in` row, and opens the browser from
Rust through `tauri-plugin-opener` at `BROWSER_SIGN_IN_URL` with the
challenge and the email; `accounts_sign_in_finish(code)` exchanges
the code and the verifier through the transport, deletes the row,
`keep_signed_in`, syncs, answers the user; `accounts_sign_in_cancel`
deletes the row; `accounts_sign_in_pending` answers the row, if one
is younger than ten minutes, for the page at launch.
`accounts_sign_up` and the old `accounts_sign_in` go. The waiting
state is a dialog on the accounts page, `AccountsSignInDialog`, like
the remove dialog: "Finish signing in in your browser", the field for
the code, Finish, Cancel, "Open the browser again", and a line saying
which email it started with; the accounts page opens it when
`accounts_sign_in_pending` answers a row. No new route, no new guard
rule, and the list of accounts stays behind it. The other shape is a
page, `/accounts/sign-in/wait`, with the same contents on a screen of
its own: a URL the deep link can later land on with the code, room
for the instructions on a phone, and the back button to leave it; at
the cost of a route, a page file, and a guard rule for a state that
lasts minutes. Either way the deep link, later, arrives in Rust and
calls the finish command itself, then tells the page. A person who signs in as someone else in
the browser gets that account's file, as the server names the user.
On the web, `AccountsSignInPage` and `AccountsSignUpPage` gain the
code step as a second form on the same page, and the finish-for-a-
device screen. The `lib/core` pair: `http-api.js` keeps
`accountsSignUp` and `accountsSignIn`, adds `accountsCode` and
`accountsCodeResend`, and rejects the four device commands;
`tauri-commands.js` the reverse, as `deviceAccounts*` is mirrored
today. In the app the router sends `/accounts/sign-in` and
`/accounts/sign-up` to `/accounts`.

Plugins: `tauri-plugin-opener` now, opened from Rust so no JavaScript
capability is needed for a URL only known at runtime;
`tauri-plugin-deep-link` with the mobile step, for universal links
only, with the association files served by the web server.

## OAuth later, what this leaves ready

- The device flow: a provider's callback lands on the server with the
  challenge carried in the OAuth `state`, proves the user from the
  provider's answer, and calls `finish_in` with `Device { challenge }`
  or `Session` like any other sign-in. No app change.
- `confirmed_at`: set by the provider's say-so at once.
- Credentials are already optional per user: `verify` treats a user
  with no password as a wrong password. A user who only ever signs in
  with Google has no credentials row.
- Still to add then: a `users_identities` table, provider and the
  provider's user id per user; the provider buttons on the web pages;
  Apple's rule that Sign in with Apple accompanies any other provider
  on iOS; whether to link a provider sign-in to an existing email.

## Build order

1. Core: the tokens columns and `confirmed_at`, `tokens` by context,
   `mailer` with `Log` and `Memory`, `mails`, the accounts functions
   and the purge of the unconfirmed, with tests on both databases.
2. Web: the routes, the mailer at start, the state struct, the code
   step on the two pages, the headless Chrome check with the code read
   from the log.
3. Device: the transport change, the `device_sign_in` table, the app
   commands, the dialog, the finish-for-a-device screen on the web
   pages, `BROWSER_SIGN_IN_URL`, `bin/dev`; checked with the desktop
   app and the code pasted.
4. The "Send a new code" button and its route.
5. Rate limits: `governor` in the state, the per-address and
   per-resend keys, `tower_governor` on the accounts routes.
6. Resend behind the feature, with `MAILER=resend`.
7. Later, each its own step: the desktop loopback and the iPhone's
   sign-in sheet, built 2026-11-20; the suggestions list.
