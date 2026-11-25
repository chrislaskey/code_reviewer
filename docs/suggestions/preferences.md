# Preferences: more of them kept with the account

Written 2026-11-25. Nothing is built; planned before it is.

The language and the theme are preferences in `accounts_preferences`,
per person and synced (`docs/design/locales.md`). Two other choices are
still kept per device, in localStorage, so every account on a phone
shares them:

- **Animations**, `app:animations` (`svelte/src/lib/animations.js`).
- **The files view**, list or grid, `app:files-view`
  (`svelte/src/files/files_view.js`).

Each could be a key of its own, as `theme` is: a `const` in
`data/accounts_preferences`, its values in `checked_value`, a field of
`Preferences`, and a follow function in the pages.

Some choices may belong to the device, not the person: animations
turned off for a slow phone, say. The general form is a per-device
override over the synced preference. Worth building only once a choice
needs it.
