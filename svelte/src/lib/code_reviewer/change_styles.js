// Colour means status and nothing else: what changed, and what is risky.
// Every class name is written out in full so Tailwind finds it.

/** @typedef {"added" | "modified" | "removed" | "renamed"} ChangeKind */

export const CHANGE_STYLES = {
  added: {
    dot: "bg-success",
    text: "text-success",
    label: "added",
    sign: "+",
  },
  modified: {
    dot: "bg-warning",
    text: "text-warning",
    label: "modified",
    sign: "~",
  },
  removed: {
    dot: "bg-error",
    text: "text-error",
    label: "removed",
    sign: "−",
  },
  renamed: {
    dot: "bg-info",
    text: "text-info",
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
    className: "text-error",
  },
  also_changed: {
    icon: "hero-link-micro",
    label: "Also changed in this review. Click to jump to it.",
    className: "text-warning",
  },
};

/** The colour of a line or chip. Stale callers stand out; the rest look the same. */
export const TONE_STYLES = {
  neutral: "border-base-300 bg-base-100",
  stale: "border-error/60 bg-error/10",
};

export const EDGE_COLORS = {
  neutral: "color-mix(in oklab, var(--color-base-content) 45%, transparent)",
  stale: "var(--color-error)",
};

export function plural(count, noun) {
  return `${count} ${noun}${count === 1 ? "" : "s"}`;
}
