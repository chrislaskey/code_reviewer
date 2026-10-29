/** The StillWaves layers, back to front. The colours and opacities are set per theme in styles.css. */
export const waveLayers = [1, 2, 3].map((n) => ({ color: `var(--wave-color-${n})`, opacity: `var(--wave-opacity-${n})` }));

export const themeColors = ["var(--color-primary)", "var(--color-secondary)", "var(--color-accent)"];

// Every daisyUI theme sets `color-scheme`, not just the one named "dark".
export function dark() {
  return getComputedStyle(document.documentElement).colorScheme === "dark";
}
