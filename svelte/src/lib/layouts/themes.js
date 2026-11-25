// The daisyUI themes and the one shown. The theme is the signed-in person's
// `theme` preference (app_core::accounts::preferences), null to follow the
// OS light or dark setting; theme_script.js applies it and keeps the last
// one shown on this device for the next start.

/** Every daisyUI theme, for the theme menu (ThemeChoices): Rust's `helpers::themes::SUPPORTED`. */
export const THEMES = [
  "light", "dark", "abyss", "acid", "aqua", "autumn", "black", "bumblebee",
  "business", "caramellatte", "cmyk", "coffee", "corporate", "cupcake",
  "cyberpunk", "dim", "dracula", "emerald", "fantasy", "forest", "garden",
  "halloween", "lemonade", "lofi", "luxury", "night", "nord", "pastel",
  "retro", "silk", "sunset", "synthwave", "valentine", "winter", "wireframe",
];

/** The theme in use, or "system" when following the OS light or dark setting. */
export function currentTheme() {
  const root = document.documentElement;
  return root.getAttribute("data-theme-source") === "system" ? "system" : (root.getAttribute("data-theme") ?? "system");
}

/**
 * Shows the pages in the person's choice, null to follow the OS setting.
 * @param {string | null} chosen
 */
export function themesFollow(chosen) {
  const theme = chosen && THEMES.includes(chosen) ? chosen : "system";
  window.dispatchEvent(new CustomEvent("app:set-theme", { detail: { theme } }));
}
