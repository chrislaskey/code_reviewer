// Sets the daisyUI theme on <html> before the first paint and switches it on
// the `phx:set-theme` events the theme menu dispatches. The choice is kept
// in localStorage under `phx:theme`, and "system" follows the OS setting.
//
// Not bundled: it has to run before the page paints, so vite.config.js
// inlines it where a page has the `<!-- theme-script -->` marker. The event
// and storage names are the Starter Pack's, so a web UI shares the choice.
(() => {
  const systemTheme = () => matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";

  const setTheme = (theme) => {
    if (theme === "system") {
      localStorage.removeItem("phx:theme");
      document.documentElement.setAttribute("data-theme", systemTheme());
      document.documentElement.setAttribute("data-theme-source", "system");
    } else {
      localStorage.setItem("phx:theme", theme);
      document.documentElement.setAttribute("data-theme", theme);
      document.documentElement.setAttribute("data-theme-source", "user");
    }
  };
  if (!document.documentElement.hasAttribute("data-theme")) {
    setTheme(localStorage.getItem("phx:theme") || "system");
  }
  window.addEventListener("storage", (e) => e.key === "phx:theme" && setTheme(e.newValue || "system"));
  window.addEventListener("phx:set-theme", (e) => setTheme(/** @type {HTMLElement} */ (e.target).dataset.phxTheme));

  matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => {
    if (document.documentElement.getAttribute("data-theme-source") === "system") {
      document.documentElement.setAttribute("data-theme", systemTheme());
    }
  });
})();
