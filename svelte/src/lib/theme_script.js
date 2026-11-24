// Sets the daisyUI theme on <html> before the first paint and switches it on
// the `app:set-theme` events the theme menu dispatches. The choice is kept
// in localStorage under `app:theme`, and "system" follows the OS setting.
// The theme's page colour is kept too, under `app:theme-background`, and
// painted on <html> before the stylesheet arrives, so the window opens in
// it rather than in white.
//
// Not bundled: it has to run before the page paints, so vite.config.js
// inlines it where index.html has the `<!-- theme-script -->` marker.
(() => {
  const root = document.documentElement;
  const THEME_KEY = "app:theme";
  const BACKGROUND_KEY = "app:theme-background";

  const systemTheme = () => (matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light");

  // Reads the page colour the stylesheet gives the theme, once it is there.
  const rememberBackground = () => {
    root.style.removeProperty("background-color");
    const background = getComputedStyle(root).backgroundColor;
    if (background && background !== "rgba(0, 0, 0, 0)") {
      localStorage.setItem(BACKGROUND_KEY, JSON.stringify({ theme: root.getAttribute("data-theme"), background }));
    }
  };

  const paintRememberedBackground = () => {
    try {
      const remembered = JSON.parse(localStorage.getItem(BACKGROUND_KEY) ?? "null");
      if (remembered?.theme === root.getAttribute("data-theme")) root.style.backgroundColor = remembered.background;
    } catch {
      localStorage.removeItem(BACKGROUND_KEY);
    }
  };

  const setTheme = (theme) => {
    if (theme === "system") {
      localStorage.removeItem(THEME_KEY);
      root.setAttribute("data-theme", systemTheme());
      root.setAttribute("data-theme-source", "system");
    } else {
      localStorage.setItem(THEME_KEY, theme);
      root.setAttribute("data-theme", theme);
      root.setAttribute("data-theme-source", "user");
    }
    rememberBackground();
  };
  if (!root.hasAttribute("data-theme")) {
    setTheme(localStorage.getItem(THEME_KEY) || "system");
    paintRememberedBackground();
  }
  window.addEventListener("load", rememberBackground);
  window.addEventListener("storage", (e) => e.key === THEME_KEY && setTheme(e.newValue || "system"));
  window.addEventListener("app:set-theme", (e) => setTheme(/** @type {HTMLElement} */ (e.target).dataset.theme));

  matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => {
    if (root.getAttribute("data-theme-source") === "system") {
      root.setAttribute("data-theme", systemTheme());
      rememberBackground();
    }
  });
})();
