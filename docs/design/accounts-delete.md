# Deleting an account

**Status: built 2026-11-12** (`purchasing-ios-build.md`, step 1; "As
built" at the end). A full user deletes their
own account from the app or the web. Apple asks for this before any
iOS release (guideline 5.1.1(v)), and `purchasing.md`'s P12 and
`purchasing-phones.md`'s M8 left it for here. The build plan is
`purchasing-ios-build.md`, step 1.

Since 2026-11-23 the server's command line deletes an account too,
`app_web accounts users delete <email>`: the same delete, with no code
and no mail, see `accounts-sign-in.md`.

## In one paragraph

The delete is the soft delete every synced row already has, done at
once to everything of the user's, so every device that pulled their
rows sees them go; their plan is cancelled in Stripe; they are signed
out everywhere; and 90 days later the purge, which already removes
soft-deleted rows, removes the user and their account outright.
It is a little more than the sketch "soft delete, sign out, purge
later" in five places: other people's devices, shared communities,
the stores, other devices of the same person, and making sure the
person meant it.

## What is already there

- `accounts_users.deleted_at` and `app_users.deleted_at`, with unique
  indexes on email only over rows not deleted, so a deleted account's
  email can sign up again at once. Sign-in already looks up only live
  accounts (`accounts_users::get_by_email`).
- The purge (`core/src/sync/purge.rs`): daily, removes rows soft-deleted
  more than `RETENTION_DAYS`, 90, ago, children first, then the
  accounts of the users it removed. The database's `ON DELETE CASCADE`
  takes the user's tokens, credentials, staff row, checkouts, purchases
  and subscriptions with them.
- The outright delete of a trial user (`trial_users.rs`), safe because
  nobody else ever saw their rows.
- `DeviceAccounts::remove` on a device, which removes an account and
  its file.

## Why not delete outright now

Other people's devices hold this user's rows: their `app_users` row
(their name and picture), their posts in shared communities, their
memberships. A row removed outright on the server leaves no deleted
row for a device to pull, so those devices would keep it for ever
(the same reason as T8 in `accounts-local-and-trial-users.md`). A soft
delete is pulled like any change, and the purge removes it once every
device has had 90 days to see it.

## What the delete does, in one transaction

1. **The account**: `accounts_users.deleted_at` set. From here sign-in
   finds no account, and the email is free to sign up again as a new
   account.
2. **The user**: `app_users` soft-deleted, name cleared to "Deleted
   user", picture removed.
3. **Their content**: every post they made, and every membership of
   theirs, soft-deleted. A community whose only live member was them,
   their `Personal` one at least, is soft-deleted with its links, and
   with every file only it had. Their other files belong to communities
   with other members and stay, see `files-communities.md`.
4. **As events**: each soft delete is an ordinary update event in the
   user's name, applied through the resource's `apply_event`, so the
   log and the pull treat it like the person deleting each row by hand.
   No write path is special.
5. **Staff**: their `ops_staff` row removed.
6. **Tokens**: kept until the purge, but refused: a token of a deleted
   account is answered `410 Gone` with `account_deleted`, not `401`, so
   a device can tell a deleted account from a sign-out (below).

After the transaction commits, outside it, the things that are not
our database:

7. **Stripe**: a live subscription cancelled at once, no refund, then
   the Stripe customer deleted, which removes the saved card. Stripe
   keeps the payments and invoices, and a charge can still be refunded
   from the dashboard. The webhooks that follow find the rows, still
   there until the purge, and change nothing that matters.
8. **The bucket**: their picture deleted at once. Their files' bytes
   stay for the files that stay; the sweep takes the others'.
9. **A mail**: "Your account was deleted", to the address it had.

If step 7 or 8 fails the delete still stands; the daily
`stripe_reconcile` cancels a live subscription of a deleted account,
and the sweeps remove bytes no live row points at.

## The person's own devices

- **The one they deleted from** removes the account and its file, the
  same as removing an account from the device's list, and opens the
  start page.
- **Their other devices** learn on their next request: the `410`
  removes that account and its file from the device, pending changes
  included, and says "This account was deleted". A `401` stays what it
  is, a sign-out elsewhere that keeps the data.

## Making sure they meant it

`/accounts/deletions/new`, linked from the account settings page:

- says what goes: everything they made, in every community, on every
  device, and that this cannot be undone;
- shows their plan, if any: a Stripe plan "ends now, with no refund";
  an App Store plan "is billed by Apple until you cancel it there",
  with Manage in the App Store; one-time purchases are lost;
- Send code mails a six-digit code, as a sign-in does, a new token
  context `DeleteCode`, ten minutes;
- the code and Delete my account finish it.

The code, not a password: a password is optional here, and the code
stops someone using a phone left signed in.

## Who may

- **A full user**: as above.
- **A trial user**: the same page with no code (no real email), and
  the delete is the trial's outright one, now.
- **A local user**: no account to delete. Removing the device's
  local data is the device's account list, not this.

## Names

| Layer | Names |
|---|---|
| Core | `core/src/accounts/deletions.rs`: `deletions::code_create(deployment, user)`, `deletions::create(deployment, stripe, user, code)` |
| API | `POST /api/accounts/deletions/code`, `POST /api/accounts/deletions` |
| Tauri | `accounts_deletions_code_create`, `accounts_deletions_create` (removes the account from the device on success) |
| Pages | `AccountsDeletionsNewPage`, at `/accounts/deletions/new` |

No migration: every column is there. `DeleteCode` is a value of the
token's text `context`.

## Decided

All seven agreed 2026-11-12 as recommended.

1. **Their content in shared communities is deleted** with them.
   Recommended, the simple and private answer. The other answer keeps
   their posts, shown as by "Deleted user", which a collaborative fork
   may want; it is the same code minus step 3's posts.
2. **No undo.** Recommended: the 90 days are for devices to see the
   delete, not a grace period; the email is free at once. An undo
   would mean keeping the email reserved and a restore path.
3. **A Stripe plan ends at once, without a refund.** Recommended; a
   refund is staff's, from the dashboard.
4. **The emailed code** to confirm. Recommended over a typed "DELETE".
5. **Other devices remove the account's data** on `410`. Recommended:
   the person asked for everything to go.
6. **An App Store plan warns, not blocks.** Apple's own guidance for
   account deletion: tell people that billing continues through Apple
   until they cancel, and how to cancel; it does not ask an app to
   refuse the delete. The page says so and opens Apple's subscription
   page (StoreKit's manage-subscriptions sheet).
7. **A mail after the delete.** Recommended.

## Left out

- Exporting one's data before deleting.
- Staff deleting someone else's account, from ops.
- Undo, see 2.
- Sign in with Apple's token revocation, if Sign in with Apple is ever
  added: Apple asks that a delete revokes it.

## As built

- **The mailer is passed in**: `deletions::code_create(deployment,
  mailer, user)` and `deletions::create(deployment, mailer, user,
  code)`, `code` none for a trial. Stripe is the deployment's
  (`Deployment::with_stripe`), as the jobs read it, not a parameter.
- **The code's row** is found by the user, `tokens::delete_code_hash`,
  with the code's hash beside it; a wrong code counts as a sign-in's
  does, and `MAX_CODE_ATTEMPTS` deletes it. A wrong code answers 422,
  never 401, which a device takes for a sign-out.
- **The user's row** is deleted by `users::delete_in`, a server write
  recorded as an event in the log, as the picture's is, since `name`
  is the only field a device may change on it.
- **The plan** answered by the current user now says its `store`, so
  the page warns by where it was bought. The App Store and Google Play
  links open each store's subscriptions page on the web until step 4
  opens StoreKit's sheet.
- **Another device** removes the account on the sync's 410; an online
  call's 410 starts a sync, so there is one path. The pages are told by
  the `synced` event's `account_deleted`, and open the device's accounts
  saying so.
- **On the web** a deleted account's cookie is read as signed out.

