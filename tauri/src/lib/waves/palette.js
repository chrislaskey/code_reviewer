export const starterPackColors = ["#e935c1", "#8b5cf6", "#3b82f6"];

export const themeColors = ["var(--color-primary)", "var(--color-secondary)", "var(--color-accent)"];

// Every daisyUI theme sets `color-scheme`, not just the one named "dark".
export function dark() {
  return getComputedStyle(document.documentElement).colorScheme === "dark";
}
