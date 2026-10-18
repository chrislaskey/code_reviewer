import type { GroupSummary } from "../api/types";
import { calleePath, fileStatus, type ChangeKind } from "./data";
import {
  childGroups,
  filesInGroup,
  findGroup,
  groupIdsForPath,
  groupPath,
  summarizeFiles,
  type GroupedFile,
  type Stats,
} from "./groups_data";

/*
 * A group drawn as one row, a level above a file. A file row has changed
 * functions in its File column, calls out as Outputs and callers as Inputs. A
 * group row has the same three columns with files (or subgroups) as content, and
 * the files outside the group that its files call or are called by.
 */

/** One line of the content column: a changed file, or a subgroup of a parent group. */
export interface GroupLine {
  /** What hovering the line ties its outputs and inputs to. */
  key: string;
  kind: "file" | "subgroup";
  /** File id, or group id. */
  id: string;
  /** File path, or subgroup label. */
  label: string;
  stats: Stats;
  /** Files only. */
  status?: ChangeKind;
}

/** A file outside the group on the other end of a call. */
export interface GroupLink {
  id: string;
  side: "input" | "output";
  path: string;
  module: string;
  /** The content line this link belongs to. */
  line_key: string;
  line_label: string;
  /** The most specific group of the other file. */
  group_id: string;
  group_label: string;
  /** Set when the other file is itself changed in this review. */
  file_id: string | null;
  call_count: number;
  /** Inputs only: still calls something removed or renamed. */
  is_stale: boolean;
  /** Inputs only: every caller is a test. */
  is_test: boolean;
}

export interface GroupRow {
  group: GroupSummary;
  /** Heading text. A subgroup shows its parents too, like `Core / Data`. */
  title: string;
  stats: Stats;
  status: ChangeKind;
  content_label: "Files" | "Subgroups";
  lines: GroupLine[];
  outputs: GroupLink[];
  inputs: GroupLink[];
  /** Calls into dependencies and the standard library, left out of the outputs. */
  folded_output_count: number;
}

function groupStatus(stats: Stats): ChangeKind {
  const { added, modified, removed, renamed } = stats.function_counts;
  if (added > 0 && modified + removed + renamed === 0) return "added";
  if (removed > 0 && added + modified + renamed === 0) return "removed";
  return "modified";
}

function mostSpecificGroup(groups: GroupSummary[], ids: string[]): GroupSummary {
  const leaf = ids.map((id) => findGroup(groups, id)).find((group) => !group.is_parent);
  return leaf ?? findGroup(groups, ids[0]);
}

function lineKeysForFile(
  groups: GroupSummary[],
  group: GroupSummary,
  file: GroupedFile,
): string[] {
  if (!group.is_parent) return [file.id];
  return childGroups(groups, group.id)
    .filter((child) => file.group_ids.includes(child.id))
    .map((child) => child.id);
}

export function buildGroupRow(
  files: GroupedFile[],
  groups: GroupSummary[],
  groupId: string,
): GroupRow {
  const group = findGroup(groups, groupId);
  const groupFiles = filesInGroup(files, groupId);
  const stats = summarizeFiles(groupFiles);

  const lines: GroupLine[] = group.is_parent
    ? childGroups(groups, groupId).map((child) => ({
        key: child.id,
        kind: "subgroup",
        id: child.id,
        label: child.label,
        stats: summarizeFiles(filesInGroup(files, child.id)),
      }))
    : groupFiles.map((file) => ({
        key: file.id,
        kind: "file",
        id: file.id,
        label: file.path,
        stats: summarizeFiles([file]),
        status: fileStatus(file),
      }));
  const lineLabel = (key: string) => lines.find((line) => line.key === key)?.label ?? key;
  const lineOrder = (key: string) => lines.findIndex((line) => line.key === key);

  const isInside = (path: string) => groupIdsForPath(files, path).includes(groupId);
  const links = new Map<string, GroupLink>();
  const addLink = (
    side: GroupLink["side"],
    key: string,
    path: string,
    module: string,
    calls: { count: number; stale: boolean; test: boolean },
  ) => {
    const id = `${side}:${key}:${path}`;
    const otherGroup = mostSpecificGroup(groups, groupIdsForPath(files, path));
    const existing = links.get(id);
    if (existing) {
      existing.call_count += calls.count;
      existing.is_stale ||= calls.stale;
      existing.is_test &&= calls.test;
      return;
    }
    links.set(id, {
      id,
      side,
      path,
      module,
      line_key: key,
      line_label: lineLabel(key),
      group_id: otherGroup.id,
      group_label: otherGroup.label,
      file_id: files.find((file) => file.path === path)?.id ?? null,
      call_count: calls.count,
      is_stale: calls.stale,
      is_test: calls.test,
    });
  };

  let foldedOutputCount = 0;
  for (const file of groupFiles) {
    for (const key of lineKeysForFile(groups, group, file)) {
      for (const callee of file.callees) {
        if (callee.origin !== "project") {
          foldedOutputCount += 1;
          continue;
        }
        const path = callee.path ?? calleePath(callee);
        if (isInside(path)) continue;
        addLink("output", key, path, callee.module, {
          count: callee.call_count,
          stale: false,
          test: false,
        });
      }
      for (const caller of file.callers) {
        if (isInside(caller.file_path)) continue;
        addLink("input", key, caller.file_path, caller.module, {
          count: 1,
          stale: caller.stale,
          test: caller.kind === "test",
        });
      }
    }
  }

  const sorted = (side: GroupLink["side"]) =>
    [...links.values()]
      .filter((link) => link.side === side)
      .sort((a, b) => lineOrder(a.line_key) - lineOrder(b.line_key));

  return {
    group,
    title: groupPath(groups, group.id).map((part) => part.label).join(" / "),
    stats,
    status: groupStatus(stats),
    content_label: group.is_parent ? "Subgroups" : "Files",
    lines,
    outputs: sorted("output"),
    inputs: sorted("input"),
    folded_output_count: foldedOutputCount,
  };
}
