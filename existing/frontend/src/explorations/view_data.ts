import type { BlockRow } from "./block";
import {
  calleeLabel,
  callerLabel,
  findFileForCallee,
  findFileForCaller,
  isOutsideProject,
  shortPath,
  splitCallees,
  type ExampleCallee,
  type ExampleCaller,
  type ExampleFile,
  type OutsideMode,
} from "./data";
import {
  calleeMarkers,
  calleeTone,
  callerMarkers,
  callerTone,
  type CardLook,
  type ChipData,
  type Marker,
} from "./nodes";

/*
 * What each caller, call and block row says. Shared by the ReactFlow views and
 * the plain HTML views, so both draw the same text.
 */

export function chipForCaller(
  files: ExampleFile[],
  file: ExampleFile,
  caller: ExampleCaller,
  options: { look?: CardLook & { pathSubtitles: boolean; subtleTones: boolean }; compact: boolean },
): ChipData {
  const { look } = options;
  const alsoIn = caller.also_changed ? findFileForCaller(files, caller) : undefined;
  return {
    side: "input",
    title: callerLabel(caller),
    subtitle: look?.pathSubtitles
      ? `${caller.file_path}:${caller.line}`
      : `${shortPath(caller.file_path)}:${caller.line} → ${caller.calls}`,
    tone: callerTone(caller),
    markers: callerMarkers(caller),
    jumpToFileId: alsoIn?.id,
    focusKey: `${file.id}:${caller.calls}`,
    compact: options.compact,
    subtle: look?.subtleTones ?? false,
    subtitleIsPath: look?.pathSubtitles,
  };
}

export function chipForCallee(
  files: ExampleFile[],
  file: ExampleFile,
  callee: ExampleCallee,
  options: { look?: CardLook & { pathSubtitles: boolean; subtleTones: boolean }; compact: boolean },
): ChipData {
  const { look } = options;
  const alsoIn = findFileForCallee(files, callee);
  const markers: Marker[] = [...calleeMarkers(callee)];
  if (alsoIn) markers.push("also_changed");
  return {
    side: "output",
    title: calleeLabel(callee),
    subtitle: look?.pathSubtitles
      ? callee.path
      : `from ${callee.from}${callee.call_count > 1 ? ` ×${callee.call_count}` : ""}`,
    tone: calleeTone(callee),
    markers,
    jumpToFileId: alsoIn?.id,
    focusKey: `${file.id}:${callee.from}`,
    dimOrigin: isOutsideProject(callee),
    compact: options.compact,
    subtle: look?.subtleTones ?? false,
    subtitleIsPath: look?.pathSubtitles,
  };
}

/** Rows for the inputs block, with a heading per changed function when grouped. */
export function blockRowsForCallers(
  files: ExampleFile[],
  file: ExampleFile,
  groupedByFunction: boolean,
): BlockRow[] {
  const rows: BlockRow[] = [];
  let previous: string | null = null;
  for (const caller of file.callers) {
    if (groupedByFunction && previous !== caller.calls) {
      rows.push({ kind: "heading", label: caller.calls, focusKey: `${file.id}:${caller.calls}` });
    }
    previous = caller.calls;
    const alsoIn = caller.also_changed ? findFileForCaller(files, caller) : undefined;
    rows.push({
      kind: "item",
      title: callerLabel(caller),
      detail: groupedByFunction ? undefined : caller.calls,
      tone: callerTone(caller),
      markers: callerMarkers(caller),
      dim: caller.kind === "test",
      focusKey: `${file.id}:${caller.calls}`,
      jumpToFileId: alsoIn?.id,
    });
  }
  return rows;
}

/** Rows for the outputs block. */
export function blockRowsForCallees(
  files: ExampleFile[],
  file: ExampleFile,
  outside: OutsideMode,
  groupedByFunction: boolean,
): BlockRow[] {
  const { shown, folded } = splitCallees(file.callees, outside);
  const rows: BlockRow[] = [];
  let previous: string | null = null;
  for (const callee of shown) {
    if (groupedByFunction && previous !== callee.from) {
      rows.push({ kind: "heading", label: callee.from, focusKey: `${file.id}:${callee.from}` });
    }
    previous = callee.from;
    const alsoIn = findFileForCallee(files, callee);
    const markers: Marker[] = [...calleeMarkers(callee)];
    if (alsoIn) markers.push("also_changed");
    const count = callee.call_count > 1 ? `×${callee.call_count}` : undefined;
    rows.push({
      kind: "item",
      title: calleeLabel(callee),
      detail: groupedByFunction ? count : (count ?? callee.from),
      tone: calleeTone(callee),
      markers,
      dim: isOutsideProject(callee),
      focusKey: `${file.id}:${callee.from}`,
      jumpToFileId: alsoIn?.id,
    });
  }
  if (folded.length > 0) {
    rows.push({ kind: "note", label: `+${folded.length} in dependencies and stdlib` });
  }
  return rows;
}

export function countItems(rows: BlockRow[]): number {
  return rows.filter((row) => row.kind === "item").length;
}
