# Navigation: what was left out

The layout follows Code Reviewer's: a gutter of tabs down the left edge
(`svelte/src/lib/layouts/AppNav.svelte`), and on small windows a bar along the
bottom with Home, Back, Forward and a Menu sheet
(`svelte/src/lib/layouts/BottomBar.svelte`). A header across the window's top
(`svelte/src/lib/layouts/AppHeader.svelte`) holds what has no place in either:
Back and Forward in the desktop app, the community switcher, the sync
status (`SyncMenu.svelte`) and the person (`AccountMenu.svelte`), with
the trial's and a failed payment's banners below it (`AppLayout.svelte`).

Other homes were drawn for these and set aside for now:

- **An account hub in the gutter**: an avatar tile at the gutter's bottom
  beside Theme, holding identity, accounts, sync and sign-out, with badges
  for what needs attention, and the community as a square at the gutter's
  top; on small windows, the top of the Menu sheet or a fifth slot in the
  bottom bar.
- **A header per page on small windows**: sticky, with the community, the
  page's title and its main action ("New post"), collapsing as the page
  scrolls; it would replace the "Back to …" links, which the bottom bar's
  Back already does. On the desktop, a sticky row atop the page with a
  breadcrumb and the page's actions.
- **The trial as a chip** in the header ("5h left") in place of the
  banner, the banner coming back only near the trial's end.

Still open:

- **The theme choices' rows** are each painted in their own theme
  (`data-theme` on the button in `ThemeChoices.svelte`); Code Reviewer
  paints only the swatch.
- **The page's width** is still 560px, against the gutter rather than
  centred; the lists might use more of a wide window.
- **"Play animations"** on the Files pages is a preference, and might sit
  beside Theme.
