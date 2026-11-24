// Colour means status and nothing else: what changed, and what is risky.
// Every class name is written out in full so Tailwind finds it.
import { t } from "ttag";

/** @typedef {"added" | "modified" | "removed" | "renamed"} ChangeKind */

export const CHANGE_STYLES = {
  added: {
    dot: "bg-success",
    text: "text-success",
    sign: "+",
  },
  modified: {
    dot: "bg-warning",
    text: "text-warning",
    sign: "~",
  },
  removed: {
    dot: "bg-error",
    text: "text-error",
    sign: "−",
  },
  renamed: {
    dot: "bg-info",
    text: "text-info",
    sign: "↔",
  },
};

/**
 * What a change is called, `added`.
 * @param {ChangeKind} kind
 */
export function changeLabel(kind) {
  if (kind === "added") return t`added`;
  if (kind === "modified") return t`modified`;
  if (kind === "removed") return t`removed`;
  if (kind === "renamed") return t`renamed`;
  return kind;
}

/** @typedef {"stale" | "also_changed"} Marker */

/** The small icons after a caller or call. */
export const MARKERS = {
  stale: {
    icon: "hero-exclamation-triangle-micro",
    className: "text-error",
  },
  also_changed: {
    icon: "hero-link-micro",
    className: "text-warning",
  },
};

/**
 * What a marker means, its icon's title.
 * @param {Marker | string} marker
 */
export function markerLabel(marker) {
  if (marker === "stale") return t`Still calls a removed or renamed function`;
  return t`Also changed in this review. Click to jump to it.`;
}

/** The colour of a line or chip. Stale callers stand out; the rest look the same. */
export const TONE_STYLES = {
  neutral: "border-base-300 bg-base-100",
  stale: "border-error/60 bg-error/10",
};

export const EDGE_COLORS = {
  neutral: "color-mix(in oklab, var(--color-base-content) 45%, transparent)",
  stale: "var(--color-error)",
};
