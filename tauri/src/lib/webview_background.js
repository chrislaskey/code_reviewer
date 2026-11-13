// Gives the webview the theme's page colour, so the strips the page doesn't
// paint (behind a phone's status bar and home indicator, past the ends of a
// page) match the theme instead of the white in tauri.conf.json.

import { setWebviewBackgroundColor } from "./backend.js";

/** Paints the webview in the theme's page colour now and whenever the theme changes. */
export function followThemeWithWebviewBackground() {
  const root = document.documentElement;
  const paint = () => {
    const color = rgbaOf(getComputedStyle(root).backgroundColor);
    if (color[3] > 0) setWebviewBackgroundColor(color).catch((error) => console.error(error));
  };
  paint();
  new MutationObserver(paint).observe(root, { attributes: true, attributeFilter: ["data-theme"] });
}

// The themes' colours are oklch, which Tauri doesn't read: a canvas turns any CSS colour into RGBA.
function rgbaOf(cssColor) {
  const context = document.createElement("canvas").getContext("2d");
  context.fillStyle = cssColor;
  context.fillRect(0, 0, 1, 1);
  return [...context.getImageData(0, 0, 1, 1).data];
}
