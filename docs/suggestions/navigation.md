# Navigation: what was left out

The layout follows Code Reviewer's: a gutter of tabs down the left edge
(`svelte/src/lib/AppNav.svelte`), and on small windows a bar along the
bottom with Home, Back, Forward and a Menu sheet
(`svelte/src/lib/BottomBar.svelte`). What has no place in either yet is
still above each page (`svelte/src/lib/PageTopBar.svelte`):

- **A home for the account**: who is signed in, Profile, Accounts on a
  device, Sign in, Sign up and Sign out, perhaps as a tile at the gutter's
  bottom beside Theme, opening a menu, and a section in the bottom bar's
  sheet.
- **A home for the community switcher**, perhaps at the gutter's top,
  where it scopes every tab below it.
- **A home for the sync status** in the app, perhaps a badge on the
  account's tile.
- **Back and Forward on the desktop**, as Code Reviewer has beside the
  window controls: the desktop app has neither a browser's buttons nor
  the bottom bar's.
- **The theme choices' rows** are each painted in their own theme
  (`data-theme` on the button in `ThemeChoices.svelte`); Code Reviewer
  paints only the swatch.
- **The page's width** is still 560px, now against the gutter rather
  than centred; the lists might use more of a wide window.
