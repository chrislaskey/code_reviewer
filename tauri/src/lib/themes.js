// Every daisyUI theme, for the theme menus (ThemeChoices).
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
  return root.getAttribute("data-theme-source") === "system" ? "system" : root.getAttribute("data-theme") ?? "system";
}
