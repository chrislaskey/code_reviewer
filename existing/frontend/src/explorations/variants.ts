import type { Edge, Node } from "@xyflow/react";
import type { ChipTone } from "./style";
import { blockHeight, type BlockData } from "./block";
import {
  calleeLabel,
  isOutsideProject,
  splitCallees,
  type ExampleCallee,
  type ExampleCaller,
  type ExampleFile,
  type OutsideMode,
} from "./data";
import { blockRowsForCallees, blockRowsForCallers, chipForCallee, chipForCaller, countItems } from "./view_data";
import { fade, sortFiles, splitFocusKey, withFade, type LedgerSort } from "./depth";
import { ledgerHeight, type LedgerData, type LedgerStyle } from "./ledger";
import { SIZES, chipItem, layoutRows, listHeight, makeEdge, type ColumnItem, type Graph } from "./layout";
import {
  calleeTone,
  callerTone,
  cardHeight,
  type CardLook,
  type CardStats,
  type FileCardData,
  type ListData,
} from "./nodes";

/*
 * Round three. Every node view here connects each side to the card at one
 * point, and shows what belongs to a function by hover instead of by drawing
 * one line per function.
 */

/* -- Ledger: E3 with options -------------------------------------------------- */

export function buildLedgerVariant(
  files: ExampleFile[],
  options: {
    style: LedgerStyle;
    outside: OutsideMode;
    sort: LedgerSort;
    collapsed: Set<string>;
    selected: string | null;
  },
): Graph {
  let y = 0;
  const nodes: Node[] = sortFiles(files, options.sort).map((file) => {
    const id = `${file.id}:ledger`;
    const data: LedgerData = {
      file,
      outside: options.outside,
      style: options.style,
      collapsed: options.collapsed.has(id),
      selectedLabel: options.selected,
    };
    const node: Node = { id, type: "ledger", position: { x: 0, y }, data };
    y += ledgerHeight(data) + SIZES.rowGap / 2;
    return node;
  });
  return { nodes, edges: [], height: Math.max(y - SIZES.rowGap / 2, 0) };
}

/* -- Shared by the node views ------------------------------------------------- */

/** Presentation choices for the chip view that the first round did not have. */
export interface ViewLook extends CardLook {
  /** Every connector solid. Dependencies are still thinner and lighter. */
  solidEdges: boolean;
  /** Changed calls are plain boxes with a marker icon, not tinted. */
  subtleTones: boolean;
  /** Every connector has the same colour, width and style, and stays put on hover. */
  uniformEdges?: boolean;
  /** Chip subtitles are the file path alone. */
  pathSubtitles: boolean;
}

export interface NodeViewOptions {
  outside: OutsideMode;
  /** The function being hovered or pinned, as `fileId:name/arity`. */
  focusKey: string | null;
  stats: CardStats;
  look?: ViewLook;
}

/** Focus only reaches the file it is in. Every other file looks the same as without focus. */
function focusKeyInFile(file: ExampleFile, focusKey: string | null): string | null {
  return splitFocusKey(focusKey)?.fileId === file.id ? focusKey : null;
}

/** The focus here is always null or inside `file`. */
function focusContext(files: ExampleFile[], focusKey: string | null) {
  const focus = splitFocusKey(focusKey);
  return {
    /** Is something in this file hovered, and is it not this function? */
    isRelated: (fn: string) => focus === null || fn === focus.fn,
    cardDimmed: false,
    focused: focus !== null,
    files,
  };
}

/*
 * Rows are cached per analysis. Hovering rebuilds only the row hovered, and the
 * rest keep the very same node and edge objects, so ReactFlow skips them.
 */
const ROW_CACHE = new WeakMap<ExampleFile[], Map<string, unknown>>();

function cachedRow<T>(files: ExampleFile[], key: unknown[], build: () => T): T {
  const cache = ROW_CACHE.get(files) ?? new Map<string, unknown>();
  ROW_CACHE.set(files, cache);
  const id = JSON.stringify(key);
  if (!cache.has(id)) cache.set(id, build());
  return cache.get(id) as T;
}

function cardItem(file: ExampleFile, options: NodeViewOptions, focusKey: string | null): ColumnItem {
  const { look } = options;
  const stats = look?.pathHeader ? "none" : options.stats;
  const data: FileCardData = { file, ports: false, focusKey, stats, look };
  return {
    node: withFade({ id: `${file.id}:card`, type: "file_card", data }, false),
    height: cardHeight(file, false, stats, look?.functionsLabel ?? true),
  };
}

function edgeWidthFor(related: boolean, focused: boolean, uniform = false): number {
  return related && focused && !uniform ? 2.75 : 1.5;
}

/** One folded list for dependencies and the standard library, as in E1. */
function foldedOutsideList(
  file: ExampleFile,
  outside: ExampleCallee[],
  related: (fn: string) => boolean,
  focused: boolean,
  dashed = true,
  uniform = false,
): { item: ColumnItem; edge: Edge } {
  const id = `${file.id}:outside`;
  const data: ListData = {
    side: "output",
    title: `Dependencies & standard library · ${outside.length}`,
    compact: true,
    items: outside.map((callee) => ({
      title: calleeLabel(callee),
      subtitle: callee.call_count > 1 ? `×${callee.call_count}` : undefined,
    })),
  };
  const dimmed = !outside.some((callee) => related(callee.from));
  return {
    item: {
      node: withFade({ id, type: "list", data }, dimmed),
      height: listHeight(outside.length, true),
    },
    edge: fade(
      makeEdge(`card->${id}`, `${file.id}:card`, id, {
        dashed,
        width: edgeWidthFor(!dimmed, focused, uniform),
      }),
      dimmed,
    ),
  };
}

/* -- Chips, one connection point ---------------------------------------------- */

const GROUP_GAP = 20;

export function buildChipsOnePoint(
  files: ExampleFile[],
  options: NodeViewOptions & { compact: boolean },
): Graph {
  const chipHeight = options.compact ? SIZES.compactChipHeight : SIZES.chipHeight;
  const itemGap = options.compact ? 6 : SIZES.chipGap;
  const look = options.look;
  const solid = look?.solidEdges ?? false;
  const subtle = look?.subtleTones ?? false;
  const uniform = look?.uniformEdges ?? false;
  /** With subtle tones only stale callers keep a coloured connector, unless all are uniform. */
  const edgeTone = (tone: ChipTone): ChipTone =>
    uniform || (subtle && tone !== "stale") ? "neutral" : tone;

  const buildChipsRow = (file: ExampleFile, fileFocus: string | null) => {
    const context = focusContext(files, fileFocus);
    const cardId = `${file.id}:card`;
    const edges: Edge[] = [];

    let previous: string | null = null;
    const left: ColumnItem[] = file.callers.map((caller: ExampleCaller, index) => {
      const id = `${file.id}:in:${index}`;
      const data = chipForCaller(files, file, caller, { look, compact: options.compact });
      const dimmed = !context.isRelated(caller.calls);
      edges.push(
        fade(
          makeEdge(`${id}->card`, id, cardId, {
            tone: edgeTone(callerTone(caller)),
            dashed: !solid && caller.kind === "test",
            width: edgeWidthFor(!dimmed, context.focused, uniform),
          }),
          dimmed,
        ),
      );
      const startsGroup = previous !== null && previous !== caller.calls;
      previous = caller.calls;
      return {
        ...chipItem(withFade({ id, type: "chip", data }, dimmed), chipHeight),
        gapBefore: startsGroup ? GROUP_GAP : itemGap,
      };
    });

    const { shown, folded } = splitCallees(file.callees, options.outside);
    previous = null;
    const right: ColumnItem[] = shown.map((callee, index) => {
      const id = `${file.id}:out:${index}`;
      const outside = isOutsideProject(callee);
      const data = chipForCallee(files, file, callee, { look, compact: options.compact });
      const dimmed = !context.isRelated(callee.from);
      const edge = makeEdge(`card->${id}`, cardId, id, {
        tone: edgeTone(calleeTone(callee)),
        dashed: !solid && (callee.diff === "removed" || outside),
        width: outside && !uniform ? 1 : edgeWidthFor(!dimmed, context.focused, uniform),
      });
      // Solid lines still show dependencies as lighter.
      const quiet =
        solid && outside && !uniform
          ? { ...edge, style: { ...edge.style, opacity: 0.45 } }
          : edge;
      edges.push(fade(quiet, dimmed));
      const startsGroup = previous !== null && previous !== callee.from;
      previous = callee.from;
      return {
        ...chipItem(withFade({ id, type: "chip", data }, dimmed), chipHeight),
        gapBefore: startsGroup ? GROUP_GAP : itemGap,
      };
    });

    if (folded.length > 0) {
      const list = foldedOutsideList(
        file,
        folded,
        context.isRelated,
        context.focused,
        !solid,
        uniform,
      );
      right.push({ ...list.item, gapBefore: GROUP_GAP });
      edges.push(list.edge);
    }

    return {
      row: { left, card: cardItem(file, options, fileFocus), right },
      edges,
    };
  };

  const rows = files.map((file) => {
    const fileFocus = focusKeyInFile(file, options.focusKey);
    const key = ["chips", file.id, options.outside, options.stats, options.compact, look, fileFocus];
    return cachedRow(files, key, () => buildChipsRow(file, fileFocus));
  });

  const layout = layoutRows(rows.map((item) => item.row));
  return { nodes: layout.nodes, height: layout.height, edges: rows.flatMap((item) => item.edges) };
}

/* -- Blocks, one connection point --------------------------------------------- */

export function buildBlocksOnePoint(
  files: ExampleFile[],
  options: NodeViewOptions & { groupedByFunction: boolean },
): Graph {
  const buildBlocksRow = (file: ExampleFile, fileFocus: string | null) => {
    const context = focusContext(files, fileFocus);
    const cardId = `${file.id}:card`;
    const edges: Edge[] = [];
    const left: ColumnItem[] = [];
    const right: ColumnItem[] = [];

    const inputRows = blockRowsForCallers(files, file, options.groupedByFunction);
    const outputRows = blockRowsForCallees(files, file, options.outside, options.groupedByFunction);
    const dimmed = context.cardDimmed;

    if (inputRows.length > 0) {
      const id = `${file.id}:inputs`;
      const data: BlockData = {
        side: "input",
        title: `Inputs · ${countItems(inputRows)}`,
        rows: inputRows,
        focus: fileFocus,
      };
      left.push({ node: { id, type: "block", data }, height: blockHeight(inputRows) });
      edges.push(fade(makeEdge(`${id}->card`, id, cardId), dimmed));
    }
    if (outputRows.length > 0) {
      const id = `${file.id}:outputs`;
      const data: BlockData = {
        side: "output",
        title: `Outputs · ${countItems(outputRows)}`,
        rows: outputRows,
        focus: fileFocus,
      };
      right.push({ node: { id, type: "block", data }, height: blockHeight(outputRows) });
      edges.push(fade(makeEdge(`card->${id}`, cardId, id), dimmed));
    }

    return { row: { left, card: cardItem(file, options, fileFocus), right }, edges };
  };

  const rows = files.map((file) => {
    const fileFocus = focusKeyInFile(file, options.focusKey);
    const key = ["blocks", file.id, options.outside, options.stats, options.groupedByFunction, fileFocus];
    return cachedRow(files, key, () => buildBlocksRow(file, fileFocus));
  });

  const layout = layoutRows(
    rows.map((item) => item.row),
    SIZES.blockWidth,
  );
  return { nodes: layout.nodes, height: layout.height, edges: rows.flatMap((item) => item.edges) };
}
