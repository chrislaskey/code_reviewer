// Colour means status and nothing else: what changed, and what is risky.
// Every class name is written out in full so Tailwind finds it.

/** @typedef {"added" | "modified" | "removed" | "renamed"} ChangeKind */

export const CHANGE_STYLES = {
  added: {
    dot: "bg-emerald-500",
    text: "text-emerald-600 dark:text-emerald-400",
    label: "added",
    sign: "+",
  },
  modified: {
    dot: "bg-amber-400",
    text: "text-amber-600 dark:text-amber-400",
    label: "modified",
    sign: "~",
  },
  removed: {
    dot: "bg-rose-500",
    text: "text-rose-600 dark:text-rose-400",
    label: "removed",
    sign: "−",
  },
  renamed: {
    dot: "bg-violet-500",
    text: "text-violet-600 dark:text-violet-400",
    label: "renamed",
    sign: "↔",
  },
};

/** @typedef {"stale" | "also_changed"} Marker */

/** The small icons after a caller or call. */
export const MARKERS = {
  stale: {
    icon: "hero-exclamation-triangle-micro",
    label: "Still calls a removed or renamed function",
    className: "text-rose-600 dark:text-rose-400",
  },
  also_changed: {
    icon: "hero-link-micro",
    label: "Also changed in this review. Click to jump to it.",
    className: "text-amber-600 dark:text-amber-400",
  },
};

/** The colour of a line or chip. Stale callers stand out; the rest look the same. */
export const TONE_STYLES = {
  neutral: "border-base-300 bg-base-100",
  stale: "border-rose-500/60 bg-rose-500/10",
};

export const EDGE_COLORS = {
  neutral: "#94a3b8",
  stale: "#f43f5e",
};

export function plural(count, noun) {
  return `${count} ${noun}${count === 1 ? "" : "s"}`;
}
