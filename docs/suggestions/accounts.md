# Accounts: what was left out

The sign-up and sign-in with an email and the emailed code, with a
password as an optional second factor (`core/src/accounts.rs`, the design in
`docs/design/accounts-sign-in.md`), and the apps signing in through the
browser, are the smallest flow that puts a real, confirmed user on every
deployment. The Phoenix app has the rest, and each is a layer on top of
this one:

- **Local users and trial users**, what
  `docs/design/accounts-local-and-trial-users.md` left out: moving a
  local user's records into an account that already has a folder on
  the device; exporting or backing up a local user's data without
  signing up; going back from a full user to a local one; files and a
  picture for trial users, with a quota; extending a trial; ops
  listing users with their kind; a device with an account whose plan
  has no sync, which belongs with purchasing.
- **More against abuse of trials**, past what step 5 of
  `docs/design/accounts-local-and-trial-users-build.md` built: a
  CAPTCHA such as Turnstile in front of "Try it now", for a fork that
  sees its cap kept full; ops showing the count of trial users
  against `TRIAL_USERS_MAX`.
- **A rate limit per user on the application's routes.** A second
  `tower_governor` layer on `/api/posts`, `/api/communities`,
  `/api/files` and `/api/sync/*`, keyed by the session token, some
  300 requests a minute, against a signed-up user hammering the API.
  It bounds speed, not how much a user stores, which is a quota per
  plan and belongs with purchasing. Needs the sync to wait and try
  again on a 429, and the buckets are per process.
- **Magic links.** Phoenix logs a user in by a link sent to the email;
  here the email carries a code. A link is the same pending sign-in
  with the pending token and the code in the URL, and on a phone it
  can open the app directly through universal links, which a tap in
  Mail opens; `starter-pack.fly.dev` can serve their file.
- **A way back for an app that locks out.** A forgotten password is
  reset after the emailed code by default, which leaves it guarding
  nothing the inbox does not already open. With
  `accounts::FORGOTTEN_PASSWORD` set to `LockedOut` it is a true second
  factor and needs another way back: recovery codes shown when the
  password is made, or staff clearing it from ops after checking who
  is asking.
- **The choice as a setting of the deployment**, or of each account,
  rather than a constant in the code.
- **Removing a password**, back to the code alone, and **signing the
  other sessions out** when it is made or changed.
- **Other second factors** beside the password, an authenticator app
  or a passkey: another entry in `accounts::SecondFactor`, its own
  step after the code.
- **One page for sign-up and sign-in**: the email first, and the name
  asked after the code when the account is new.
- **Changing the email**, with a confirmation link, which also has to
  update the synced `app_users` row.
- **Sudo mode**: asking for the password again before a sensitive
  change, as Phoenix does within twenty minutes of a sign-in.
- **Deleting an account**, see the Phoenix build order's phase 7.
- **Keeping a browser signed in across its sessions.** The cookie lasts
  the browser session; a "remember me" would give it the token's year.
- **Signing out everywhere**: deleting every token of the user, not
  just the one the request carried, and a page listing the tokens.
- **Signing out, or removing an account, while offline.** A device
  cannot sign out without reaching the server to forget its token, and
  Remove from device signs out first, so a person offline cannot hand
  the device over. The alternative, dropping the data and leaving the
  token to expire, is a choice to make deliberately.
- **Encrypting the account files.** A signed-out account's file stays
  readable on disk until it is removed, the same exposure the active
  account has; SQLCipher would close it for every file.
- **Returning to the page an account was last on** after a switch,
  instead of the landing page.
- **Remembering a browser** for thirty days, so the code is not asked
  on every new browser session: Phoenix's remember-me, a longer cookie
  on the `session` token.
- **The automatic return on Android**, in place of pasting the code:
  a Custom Tab, Android's sign-in sheet, as the iPhone's
  `ASWebAuthenticationSession`. The desktop's loopback and the iPhone's
  sheet are built, see "Since 2026-11-20" in the design.
- **A dev mailbox page** showing the emails the `Memory` mailer holds,
  as Phoenix's `/dev/mailbox` does, instead of reading the log.
- **HTML emails and a sender name**; the emails are text only.
- **Continue as the person signed in here** on the sign-in page a device
  opened, when the browser already has a session.
- **Rate limits shared between several servers**: `governor`'s buckets
  are this process's; several copies of the server need a shared store
  or a limit at the proxy.
