# Locales

The language the pages are shown in: a person's choice, kept with their
account and synced to their devices, else their device's language. The
pages' text is gettext through ttag, see the README's "Translations".

## Decided

- **The choice is per person, not per device.** Picking Español on the
  laptop shows the phone in Español after its next sync. With no choice,
  the pages follow the device: the browser's languages on the web, the
  system's in the apps, including a phone's per-app language setting.
- **One picker everywhere:** the profile page's Language, "Follow the
  system" and then each language in its own name. It shows once the
  pages are in more than one language.
- **Translations are made by an LLM, then reviewed by a person.** A
  message in a `.po` that is missing, untranslated or fuzzy fails
  `bin/lint`, so a change that adds English carries its translations.

## Where the choice is kept: `accounts_preferences`

A person's preferences are one row per user and key, `locale` the first,
its value JSON: a language of `helpers::locales::SUPPORTED`, or null to
follow the device. `core/src/accounts/preferences.rs` reads them as
`Preferences` and writes one key at a time; `data/accounts_preferences`
holds the keys and the values each takes.

**A preference lives with the lowest layer that reads it.** Every app
may read a person's language, and `accounts` is the lowest of them: it
mails people, and will mail them with nobody on a page (a renewal that
failed, say). `accounts` never reads an `app_` table, see
`multi-app.md`, so the preferences are an accounts table, which every
app may read.

**It is the one accounts table that syncs**, to its own user's devices
and nobody else's, because the pages read it on every start, offline
too, and a device changes it offline. The rule that only `app_` tables
sync keeps emails, tokens and other server facts on the server; a
person's own choices, written by them and checked by the server, are
neither, and one named exception keeps the prefix readable. It syncs
as any table does, through `SyncedTable`, listed in
`app/synced_tables.rs` after `app_users`:

- **Every write is an event**, as a name change is: a create for a key
  the user has not set, an update of its `value` after. The server
  checks the row is the writer's and the value is one the key takes.
- **The last write the server applies wins.** A preference has no
  conflicts. Two devices that each made the same key offline make two
  rows; the server writes the second's value over the first's row and
  answers with it, and the device drops its own, as with a membership
  revived under another id.
- **A device holds its user's rows alone**: the pull's visibility is
  `user_id` alone.
- **No foreign key.** A device's `accounts_users` is empty and a local
  user has no account, so a key to it fails, and one to `app_users`
  would make accounts depend on the app. The rows go with their
  account instead: `accounts::deletions` deletes them, and the purge
  removes any whose account is gone, a trial's or an unconfirmed
  sign-up's.
- **A local user's rows move with them** to the account they sign up
  for, as their posts do; the creates then write over the account's.

## How the pages follow it

`svelte/src/lib/translations.js`:

- **Before the first page draws**, `translationsStart` loads the
  language last chosen on this device, kept in localStorage under
  `app:locale`, else the device's: its first language the pages are in,
  `es-MX` as `es`, else English. `main.js` and the quick picker's wait
  for it. It sets `<html lang>`.
- **The current user carries their preferences**, `GET
  /api/users/current` and `current_user_get`. `translationsFollow`
  keeps the choice for the next start and reloads the page once when
  it shows another language, a choice made on another device or a
  first sign-in. Every `t` runs as the page draws, so a reload is what
  shows the new language.
- **Choosing** is `accountsPreferencesUpdate("locale", value)`, `PUT
  /api/accounts/current/preferences` or `accounts_preferences_update`,
  written on the device and synced, then `translationsFollow`.

The public website is in the same languages, at `/es/about`, from a
catalog of its own, see `public-pages.md`.

The languages are `helpers::locales::SUPPORTED`, English first, and a
`<locale>.po` each in `svelte/translations/`,
`svelte/translations/public/` and `core/translations/`; core tests fail
when they differ. Adding one: `msginit` the three `.po` files (README),
translate them, `pnpm translations:update`, add its code to `SUPPORTED`
and its `.mo` to `helpers::translations::CATALOGS`.

## Emails

An email goes in the person's language: their `locale` preference, else
the language the request that sent it asked for, its `Accept-Language`
matched by `helpers::locales::resolve`, else English
(`accounts::preferences::mail_locale_in`). The web server reads the
header with `RequestLocale`; the apps' own requests send none, so a
person who never chose gets those in English, which is fine. Every mail
function in `services/mailer/mails.rs` takes the language, and each
sender of one takes `requested`.

Rust's text has a catalog of its own, `core/translations/`, the third
beside the pages' two, so none writes over another:

- **The English** is written in `t!(locale, "…", name = value)`
  (`helpers::translations`), named placeholders in braces, the app's
  name `{app}`.
- **`pnpm translations:extract`** reads every `t!` in `core/src` into
  `default.pot` (`svelte/scripts/rust_translations.js`), and
  **`translations:update`** merges each `<locale>.po` and compiles it to
  the `<locale>.mo` the `gettext` crate reads, committed, embedded with
  `include_bytes!`: no gettext tool is needed to build.
- **`translations:check`** fails on a `.pot` out of date, a message
  untranslated or fuzzy, a `.mo` not compiled from its `.po`, and a
  translation whose placeholders differ from the English's.

## Later
- **Errors in the person's language.** The server sends a code and its
  values (`posts.body_too_long`, `{max}`) beside the English, and the
  pages translate the code; `ApiError` has a `code` already.
- **Dates and money in the language shown.** `Intl` takes the browser's
  language today, and a post's time shows as stored.
- **The staff's own catalog** for `ops`, kept out of the app's today.
- **Strings in `<script>`**, which `translations:check` does not read.
