import type { ReactNode } from "react";
import { ExclamationTriangleIcon } from "@heroicons/react/16/solid";
import { FolderIcon } from "@heroicons/react/20/solid";
import type { GroupSummary } from "../api/types";
import { HtmlReview } from "../review/html/HtmlReview";
import { fileStatus, type ExampleFile } from "./data";
import {
  UNGROUPED_ID,
  findGroup,
  summarizeFiles,
  type GroupedFile,
  type Stats,
} from "./groups_data";
import { CoverageDot, plural } from "./nodes";
import { CHANGE_STYLES } from "./style";

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

/** Changed lines as a bar: green for added, red for removed. `max` is the widest bar's lines. */
export function ChangeBar({ stats, max }: { stats: Stats; max: number }) {
  const total = stats.added_lines + stats.removed_lines;
  const width = max === 0 ? 0 : (total / max) * 100;
  return (
    <span
      title={`+${stats.added_lines} −${stats.removed_lines}`}
      className="block h-2 overflow-hidden rounded-full bg-base-content/10"
    >
      <span className="flex h-full" style={{ width: `${width}%` }}>
        <span className="h-full bg-emerald-500" style={{ flexGrow: stats.added_lines }} />
        <span className="h-full bg-rose-500" style={{ flexGrow: stats.removed_lines }} />
      </span>
    </span>
  );
}

/** A group's name as a small tag. Ungrouped is dashed because nobody defined it. */
export function GroupPill({ group }: { group: GroupSummary }) {
  return (
    <span
      title={group.id}
      className={[
        "inline-flex shrink-0 items-center rounded-full border px-2 py-px text-[10px] leading-4",
        group.id === UNGROUPED_ID
          ? "border-dashed border-base-content/30 text-base-content/50"
          : "border-base-300 bg-base-100 text-base-content/70",
      ].join(" ")}
    >
      {group.label}
    </span>
  );
}

/** Every group a file is in. A subgroup's parent is left out, since it is implied. */
export function GroupPills({ file, groups }: { file: GroupedFile; groups: GroupSummary[] }) {
  const own = file.group_ids
    .map((id) => findGroup(groups, id))
    .filter((group) => !group.is_parent);
  return (
    <span className="inline-flex flex-wrap gap-1">
      {own.map((group) => (
        <GroupPill key={group.id} group={group} />
      ))}
    </span>
  );
}

/** One file as a summary line: status, path, function counts, lines, warnings, coverage. */
export function FileSummaryLine({
  file,
  leading,
  trailing,
  onClick,
}: {
  file: ExampleFile;
  leading?: ReactNode;
  trailing?: ReactNode;
  onClick?: () => void;
}) {
  const stats = summarizeFiles([file]);
  const status = CHANGE_STYLES[fileStatus(file)];
  const content = (
    <>
      {leading}
      <span
        title={`File ${status.label}`}
        className={`size-2 shrink-0 rounded-full ${status.dot}`}
      />
      <span className="min-w-0 flex-1">
        <span className="block truncate font-mono text-xs">{file.path}</span>
        <span className="block truncate font-mono text-[10px] text-base-content/50">
          {file.module}
        </span>
      </span>
      {trailing}
      <span className="shrink-0 font-mono text-[11px] text-base-content/70">
        {plural(stats.function_count, "function")}
      </span>
      <LinesLabel added={stats.added_lines} removed={stats.removed_lines} />
      <span className="w-4 shrink-0">
        <StaleIcon count={stats.stale_caller_count} />
      </span>
      <span className="w-10 shrink-0">
        {file.coverage_percent !== undefined && <CoverageDot percent={file.coverage_percent} />}
      </span>
    </>
  );
  const className = "flex w-full items-center gap-3 px-3 py-2 text-left";
  return onClick ? (
    <button type="button" onClick={onClick} className={`${className} hover:bg-base-200/70`}>
      {content}
    </button>
  ) : (
    <div className={className}>{content}</div>
  );
}

/** The existing review for these files: one ledger row per file, functions and calls. */
export function ZoomedReview({ files }: { files: ExampleFile[] }) {
  return <HtmlReview files={files} view="ledger" outside="folded" />;
}

/** Shown wherever the groups section is empty because no groups are defined. */
export function NoGroupsNote({ children }: { children: ReactNode }) {
  return (
    <p className="border-t border-base-300 bg-base-100 px-4 py-2 text-[11px] text-base-content/60">
      {children}
    </p>
  );
}

const FOLDER_PATH =
  "M2 3.5A1.5 1.5 0 0 1 3.5 2h2.879a1.5 1.5 0 0 1 1.06.44l1.122 1.12A1.5 1.5 0 0 0 9.62 4H12.5A1.5 1.5 0 0 1 14 5.5v1.401a2.986 2.986 0 0 0-1.5-.401h-9c-.546 0-1.059.146-1.5.401V3.5ZM2 9.5v3A1.5 1.5 0 0 0 3.5 14h9a1.5 1.5 0 0 0 1.5-1.5v-3A1.5 1.5 0 0 0 12.5 8h-9A1.5 1.5 0 0 0 2 9.5Z";

/** Two folders, one behind the other: a group made of subgroups. */
export function FoldersIcon({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 16 16" fill="currentColor" aria-hidden="true" className={className}>
      <g transform="translate(5 -0.5) scale(0.7)" opacity="0.5">
        <path d={FOLDER_PATH} />
      </g>
      <g transform="translate(-0.5 3) scale(0.82)">
        <path d={FOLDER_PATH} />
      </g>
    </svg>
  );
}

/** One folder for a group with patterns, two for a parent group. */
export function groupIcon(group: GroupSummary) {
  return group.is_parent ? FoldersIcon : FolderIcon;
}
