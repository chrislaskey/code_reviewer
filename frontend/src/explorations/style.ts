import type { ChangeKind } from "./data";

/** Colour means status and nothing else: what changed, and what is risky. */
export const CHANGE_STYLES: Record<
  ChangeKind,
  { dot: string; text: string; name: string; label: string; sign: string }
> = {
  added: {
    dot: "bg-emerald-500",
    text: "text-emerald-600 dark:text-emerald-400",
    name: "text-emerald-700 dark:text-emerald-300",
    label: "added",
    sign: "+",
  },
  modified: {
    dot: "bg-amber-400",
    text: "text-amber-600 dark:text-amber-400",
    name: "text-amber-700 dark:text-amber-300",
    label: "modified",
    sign: "~",
  },
  removed: {
    dot: "bg-rose-500",
    text: "text-rose-600 dark:text-rose-400",
    name: "text-rose-700 dark:text-rose-300",
    label: "removed",
    sign: "−",
  },
  renamed: {
    dot: "bg-violet-500",
    text: "text-violet-600 dark:text-violet-400",
    name: "text-violet-700 dark:text-violet-300",
    label: "renamed",
    sign: "↔",
  },
};

export type ChipTone = "neutral" | "test" | "stale" | "new" | "removed";

export const TONE_STYLES: Record<ChipTone, string> = {
  neutral: "border-base-300 bg-base-100",
  test: "border-dashed border-base-content/30 bg-base-100/60",
  stale: "border-rose-500/60 bg-rose-500/10",
  new: "border-emerald-500/50 bg-emerald-500/10",
  removed: "border-dashed border-rose-500/50 bg-rose-500/5",
};

/** Quieter chips: every tone looks the same. The marker icon says what changed. */
export const SUBTLE_TONE_STYLES: Record<ChipTone, string> = {
  neutral: "border-base-300 bg-base-100",
  test: "border-base-300 bg-base-100 text-base-content/70",
  stale: "border-base-300 bg-base-100",
  new: "border-base-300 bg-base-100",
  removed: "border-base-300 bg-base-100",
};

export const EDGE_COLORS: Record<ChipTone, string> = {
  neutral: "#94a3b8",
  test: "#94a3b8",
  stale: "#f43f5e",
  new: "#10b981",
  removed: "#f43f5e",
};
