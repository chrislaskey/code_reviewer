# Quick picker: what was left out

Built from `docs/design/quick-picker.md`. Left for later, on purpose:

- **A shortcut the person chooses.** `unregister` and `register` on the
  global shortcut plugin, the choice kept in `device_settings`.
- **What is listed.** Images only; posts (pasted as their body),
  snippets of their own, or files that are not images (copied as the
  file) would each be a search of their own beside the files'.
- **Small thumbnails**, made once and kept, instead of the whole image.
- **The theme, live.** The page would listen for the theme's change
  (a `storage` event) as the main window makes it.
- **No Dock icon and a tray icon**, so the picker works with the main
  window closed: `ActivationPolicy::Accessory`.
- **A translucent background** like Spotlight's: `macos-private-api`
  and `NSVisualEffectView`.
- **Pasteboard markers.** `org.nspasteboard.source` on every copy, so
  clipboard managers treat it right.
- **Windows and Linux.** `unavailable.rs` is where their popup goes.
