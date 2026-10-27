import type { CSSProperties } from "react";
import type { Edge, Node } from "@xyflow/react";
import {
  calleeLabel,
  callerLabel,
  findFileForCallee,
  findFileForCaller,
  functionKey,
  groupBy,
  shortPath,
  type ExampleCallee,
  type ExampleCaller,
  type ExampleFile,
} from "./data";
import {
  SIZES,
  chipItem,
  layoutRows,
  listHeight,
  makeEdge,
  type ColumnItem,
  type Graph,
  type RowLayout,
} from "./layout";
import {
  calleeMarkers,
  calleeTone,
  callerMarkers,
  callerTone,
  cardHeight,
  plural,
  rowHeight,
  type ChipData,
  type FileCardData,
  type GroupData,
  type ListData,
  type Marker,
  type RowData,
} from "./nodes";
import type { ChipTone } from "./style";

const DIMMED_OPACITY = 0.2;
const FADE = "";

export function fade<T extends { style?: CSSProperties }>(item: T, dimmed: boolean): T {
  if (!dimmed) return item;
  return { ...item, style: { ...item.style, opacity: DIMMED_OPACITY } };
}

export function withFade(node: Omit<Node, "position">, dimmed: boolean): Omit<Node, "position"> {
  return fade({ ...node, className: FADE }, dimmed);
}

function edgeWidth(count: number): number {
  return Math.min(1.5 + (count - 1) * 1.5, 6);
}

/** Dependencies and the standard library: calls that rarely need review. */
function isOutsideProject(callee: ExampleCallee): boolean {
  return callee.origin !== "project";
}

/* -- Review canvas: ports, markers, diff, collapsed dependencies, focus ----- */

/** `fileId:name/arity`, as the file card reports it. */
export type FocusKey = string;

export function splitFocusKey(key: FocusKey | null): { fileId: string; fn: string } | null {
  if (!key) return null;
  const at = key.indexOf(":");
  return { fileId: key.slice(0, at), fn: key.slice(at + 1) };
}

export function buildReviewCanvas(files: ExampleFile[], focusKey: FocusKey | null): Graph {
  const focus = splitFocusKey(focusKey);

  const rows = files.map((file) => {
    const inFocusFile = focus?.fileId === file.id;
    const isRelated = (fn: string) => focus === null || (inFocusFile && fn === focus.fn);
    const card = `${file.id}:card`;
    const edges: Edge[] = [];

    const left = file.callers.map((caller, index) => {
      const id = `${file.id}:in:${index}`;
      const alsoIn = caller.also_changed ? findFileForCaller(files, caller) : undefined;
      const data: ChipData = {
        side: "input",
        title: callerLabel(caller),
        subtitle: `${shortPath(caller.file_path)}:${caller.line} → ${caller.calls}`,
        tone: callerTone(caller),
        markers: callerMarkers(caller),
        jumpToFileId: alsoIn?.id,
        focusKey: `${file.id}:${caller.calls}`,
      };
      const dimmed = !isRelated(caller.calls);
      edges.push(
        fade(
          makeEdge(`${id}->card`, id, card, {
            targetHandle: `in:${caller.calls}`,
            tone: callerTone(caller),
            dashed: caller.kind === "test",
          }),
          dimmed,
        ),
      );
      return {
        node: withFade({ id, type: "chip", data }, dimmed),
        height: SIZES.chipHeight,
      };
    });

    const right: ColumnItem[] = [];
    file.callees.forEach((callee, index) => {
      if (isOutsideProject(callee)) return;
      const id = `${file.id}:out:${index}`;
      const alsoIn = findFileForCallee(files, callee);
      const markers: Marker[] = [...calleeMarkers(callee)];
      if (alsoIn) markers.push("also_changed");
      const data: ChipData = {
        side: "output",
        title: calleeLabel(callee),
        subtitle: `from ${callee.from}${callee.call_count > 1 ? ` ×${callee.call_count}` : ""}`,
        tone: calleeTone(callee),
        markers,
        jumpToFileId: alsoIn?.id,
        focusKey: `${file.id}:${callee.from}`,
      };
      const dimmed = !isRelated(callee.from);
      edges.push(
        fade(
          makeEdge(`card->${id}`, card, id, {
            sourceHandle: `out:${callee.from}`,
            tone: calleeTone(callee),
            dashed: callee.diff === "removed",
          }),
          dimmed,
        ),
      );
      right.push({ node: withFade({ id, type: "chip", data }, dimmed), height: SIZES.chipHeight });
    });

    const outside = file.callees.filter(isOutsideProject);
    if (outside.length > 0) {
      const id = `${file.id}:outside`;
      const data: ListData = {
        side: "output",
        title: `Dependencies & standard library · ${outside.length}`,
        compact: true,
        items: outside.map((callee) => ({
          title: calleeLabel(callee),
          subtitle: callee.call_count > 1 ? `×${callee.call_count}` : undefined,
          tone: calleeTone(callee),
          markers: calleeMarkers(callee),
        })),
      };
      const dimmed = !outside.some((callee) => isRelated(callee.from));
      for (const from of new Set(outside.map((callee) => callee.from))) {
        edges.push(
          fade(
            makeEdge(`${id}:${from}`, card, id, {
              sourceHandle: `out:${from}`,
              dashed: true,
              width: 1,
            }),
            !isRelated(from),
          ),
        );
      }
      right.push({
        node: withFade({ id, type: "list", data }, dimmed),
        height: listHeight(outside.length, true),
      });
    }

    const cardData: FileCardData = { file, ports: true, focusKey, tabs: true };
    const row: RowLayout = {
      left,
      card: {
        node: withFade({ id: card, type: "file_card", data: cardData }, focus !== null && !inFocusFile),
        height: cardHeight(file, true),
      },
      right,
    };
    return { row, edges };
  });

  const layout = layoutRows(rows.map((item) => item.row));
  return {
    nodes: layout.nodes,
    height: layout.height,
    edges: rows.flatMap((item) => item.edges),
  };
}

/* -- Grouped by module, expand on click ------------------------------------- */

function aggregateTone(tones: ChipTone[]): ChipTone {
  if (tones.includes("stale")) return "stale";
  return tones.length > 0 && tones.every((tone) => tone === "test") ? "test" : "neutral";
}

function aggregateMarkers(markers: Marker[][]): Marker[] {
  const flat = new Set(markers.flat());
  return (["stale", "new", "removed"] as Marker[]).filter((marker) => flat.has(marker));
}

/** Group ids are `fileId:in-group:Module` and `fileId:out-group:Module`. */
export function buildGroupedExpandable(files: ExampleFile[], expanded: Set<string>): Graph {
  const rows = files.map((file) => {
    const card = `${file.id}:card`;
    const edges: Edge[] = [];
    const left: ColumnItem[] = [];
    const right: ColumnItem[] = [];

    for (const [module, callers] of groupBy(file.callers, (c) => c.module)) {
      const groupId = `${file.id}:in-group:${module}`;
      const isExpanded = expanded.has(groupId);
      const individual = (caller: ExampleCaller, index: number) => {
        const id = `${groupId}:${index}`;
        const data: ChipData = {
          side: "input",
          title: callerLabel(caller),
          subtitle: `${shortPath(caller.file_path)}:${caller.line} → ${caller.calls}`,
          tone: callerTone(caller),
          markers: callerMarkers(caller),
        };
        edges.push(
          makeEdge(`${id}->card`, id, card, {
            targetHandle: `in:${caller.calls}`,
            tone: callerTone(caller),
            dashed: caller.kind === "test",
          }),
        );
        left.push(chipItem({ id, type: "chip", data, className: FADE }));
      };

      if (callers.length === 1) {
        individual(callers[0], 0);
        continue;
      }
      const data: GroupData = {
        side: "input",
        title: module,
        detail: [...new Set(callers.map((caller) => `${caller.name}/${caller.arity}`))].join(", "),
        count: callers.length,
        expandable: true,
        expanded: isExpanded,
        tone: aggregateTone(callers.map(callerTone)),
        markers: aggregateMarkers(callers.map(callerMarkers)),
      };
      left.push(chipItem({ id: groupId, type: "module_group", data }));
      if (isExpanded) {
        callers.forEach(individual);
      } else {
        for (const [target, matches] of groupBy(callers, (caller) => caller.calls)) {
          edges.push(
            makeEdge(`${groupId}->${target}`, groupId, card, {
              targetHandle: `in:${target}`,
              width: edgeWidth(matches.length),
              tone: aggregateTone(matches.map(callerTone)),
            }),
          );
        }
      }
    }

    for (const [module, callees] of groupBy(file.callees, (c) => c.module)) {
      const groupId = `${file.id}:out-group:${module}`;
      const isExpanded = expanded.has(groupId);
      const outside = isOutsideProject(callees[0]);
      const individual = (callee: ExampleCallee, index: number) => {
        const id = `${groupId}:${index}`;
        const data: ChipData = {
          side: "output",
          title: calleeLabel(callee),
          subtitle: `from ${callee.from}${callee.call_count > 1 ? ` ×${callee.call_count}` : ""}`,
          tone: calleeTone(callee),
          markers: calleeMarkers(callee),
          dimOrigin: outside,
        };
        edges.push(
          makeEdge(`card->${id}`, card, id, {
            sourceHandle: `out:${callee.from}`,
            tone: calleeTone(callee),
            dashed: callee.diff === "removed",
          }),
        );
        right.push(chipItem({ id, type: "chip", data }));
      };

      if (callees.length === 1) {
        individual(callees[0], 0);
        continue;
      }
      const data: GroupData = {
        side: "output",
        title: module,
        detail: [...new Set(callees.map((callee) => functionKey(callee)))].join(", "),
        count: callees.reduce((sum, callee) => sum + callee.call_count, 0),
        expandable: true,
        expanded: isExpanded,
        markers: aggregateMarkers(callees.map(calleeMarkers)),
        dimOrigin: outside,
      };
      right.push(chipItem({ id: groupId, type: "module_group", data }));
      if (isExpanded) {
        callees.forEach(individual);
      } else {
        for (const [from, matches] of groupBy(callees, (callee) => callee.from)) {
          const count = matches.reduce((sum, callee) => sum + callee.call_count, 0);
          edges.push(
            makeEdge(`${groupId}<-${from}`, card, groupId, {
              sourceHandle: `out:${from}`,
              width: edgeWidth(count),
              dashed: outside,
            }),
          );
        }
      }
    }

    const cardData: FileCardData = { file, ports: true };
    return {
      row: {
        left,
        card: { node: { id: card, type: "file_card", data: cardData }, height: cardHeight(file) },
        right,
      },
      edges,
    };
  });

  const layout = layoutRows(rows.map((item) => item.row));
  return {
    nodes: layout.nodes,
    height: layout.height,
    edges: rows.flatMap((item) => item.edges),
  };
}

/* -- Ledger, refined: sort by risk, collapse rows, select a line ------------ */

export type LedgerSort = "path" | "risk" | "given";

export function riskScore(file: ExampleFile): number {
  const stale = file.callers.filter((caller) => caller.stale).length;
  const lost = file.functions.filter((fn) => fn.kind === "removed" || fn.kind === "renamed").length;
  return stale * 10 + lost * 5 + file.callers.length;
}

export function sortFiles(files: ExampleFile[], sort: LedgerSort): ExampleFile[] {
  if (sort === "given") return [...files];
  return [...files].sort((a, b) =>
    sort === "risk" ? riskScore(b) - riskScore(a) : a.path.localeCompare(b.path),
  );
}

export function buildLedgerRefined(
  files: ExampleFile[],
  options: { sort: LedgerSort; collapsed: Set<string>; selected: string | null },
): Graph {
  let y = 0;
  const nodes: Node[] = sortFiles(files, options.sort).map((file) => {
    const id = `${file.id}:row`;
    const collapsed = options.collapsed.has(id);
    const data: RowData = { file, refined: true, collapsed, selectedLabel: options.selected };
    const node: Node = { id, type: "row", position: { x: 0, y }, data };
    y += rowHeight(file, collapsed) + SIZES.rowGap / 2;
    return node;
  });
  return { nodes, edges: [], height: y };
}

export interface ChangeSummary {
  files: number;
  changed_functions: number;
  callers: number;
  stale_callers: number;
  new_project_calls: number;
}

export function summarize(files: ExampleFile[]): ChangeSummary {
  return {
    files: files.length,
    changed_functions: files.reduce((sum, file) => sum + file.functions.length, 0),
    callers: files.reduce((sum, file) => sum + file.callers.length, 0),
    stale_callers: files.reduce(
      (sum, file) => sum + file.callers.filter((caller) => caller.stale).length,
      0,
    ),
    new_project_calls: files.reduce(
      (sum, file) =>
        sum + file.callees.filter((c) => c.origin === "project" && c.diff === "new").length,
      0,
    ),
  };
}

export function summaryLabels(summary: ChangeSummary): { label: string; alert: boolean }[] {
  return [
    { label: plural(summary.files, "file"), alert: false },
    { label: plural(summary.changed_functions, "changed function"), alert: false },
    { label: plural(summary.callers, "caller"), alert: false },
    { label: `${plural(summary.new_project_calls, "new project call")}`, alert: false },
    { label: `${plural(summary.stale_callers, "stale caller")}`, alert: summary.stale_callers > 0 },
  ];
}
