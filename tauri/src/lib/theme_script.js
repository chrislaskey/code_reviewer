// Sets the daisyUI theme on <html> before the first paint and switches it on
// the `phx:set-theme` events the theme menu dispatches. The choice is kept
// in localStorage under `phx:theme`, and "system" follows the OS setting.
// The theme's page colour is kept too, under `phx:theme-background`, and
// painted on <html> before the stylesheet arrives, so the window opens in
// it rather than in white.
//
// Not bundled: it has to run before the page paints, so vite.config.js
// inlines it where index.html has the `<!-- theme-script -->` marker. The
// event and storage names are the Starter Pack's, so a web UI shares the choice.
(() => {
  const root = document.documentElement;
  const BACKGROUND_KEY = "phx:theme-background";

  const systemTheme = () => matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";

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
      localStorage.removeItem("phx:theme");
      root.setAttribute("data-theme", systemTheme());
      root.setAttribute("data-theme-source", "system");
    } else {
      localStorage.setItem("phx:theme", theme);
      root.setAttribute("data-theme", theme);
      root.setAttribute("data-theme-source", "user");
    }
    rememberBackground();
  };
  if (!root.hasAttribute("data-theme")) {
    setTheme(localStorage.getItem("phx:theme") || "system");
    paintRememberedBackground();
  }
  window.addEventListener("load", rememberBackground);
  window.addEventListener("storage", (e) => e.key === "phx:theme" && setTheme(e.newValue || "system"));
  window.addEventListener("phx:set-theme", (e) => setTheme(/** @type {HTMLElement} */ (e.target).dataset.phxTheme));

  matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => {
    if (root.getAttribute("data-theme-source") === "system") {
      root.setAttribute("data-theme", systemTheme());
      rememberBackground();
    }
  });
})();
