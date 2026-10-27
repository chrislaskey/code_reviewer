import { ExclamationTriangleIcon, RectangleStackIcon } from "@heroicons/react/16/solid";
import { FolderIcon } from "@heroicons/react/20/solid";
import type { GroupSummary } from "../api/types";
import { ALL_CHANGES_ID, type Stats } from "./groups_data";
import { plural } from "./nodes";

/* Small pieces the group explorations share. */

export function LinesLabel({ added, removed }: { added: number; removed: number }) {
  return (
    <span className="font-mono text-[11px]" title="Lines added and removed">
      <span className="text-emerald-600 dark:text-emerald-400">+{added}</span>{" "}
      <span className="text-rose-600 dark:text-rose-400">−{removed}</span>
    </span>
  );
}

/** A bare warning sign. The Inputs of the group or file say which callers are stale. */
export function StaleIcon({ count }: { count: number }) {
  if (count === 0) return null;
  return (
    <span
      title={`${plural(count, "caller")} still calling a removed or renamed function`}
      className="inline-flex shrink-0 text-rose-600 dark:text-rose-400"
    >
      <ExclamationTriangleIcon className="size-3.5" />
    </span>
  );
}

/** What a group rollup shows: its files and the lines changed. Details come when you open it. */
export function StatsLine({ stats, showFiles = true }: { stats: Stats; showFiles?: boolean }) {
  return (
    <span className="inline-flex flex-wrap items-center gap-x-3 gap-y-1">
      {showFiles && (
        <span className="font-mono text-[11px] text-base-content/70">
          {plural(stats.file_count, "file")}
        </span>
      )}
      <LinesLabel added={stats.added_lines} removed={stats.removed_lines} />
      <StaleIcon count={stats.stale_caller_count} />
    </span>
  );
}

/** One folder for a group, a stack for the All changes row. */
export function groupIcon(group: GroupSummary) {
  return group.id === ALL_CHANGES_ID ? RectangleStackIcon : FolderIcon;
}
