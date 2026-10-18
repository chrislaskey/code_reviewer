import { MarkerType, type Edge, type Node } from "@xyflow/react";
import { EDGE_COLORS, type ChipTone } from "./style";

export const SIZES = {
  chipWidth: 264,
  blockWidth: 310,
  chipHeight: 40,
  compactChipHeight: 28,
  chipGap: 10,
  cardWidth: 460,
  columnGap: 130,
  rowGap: 56,
  listHeader: 28,
  listItem: 26,
  listCompactItem: 20,
  listPadding: 8,
  /** Where edges meet every node, from its top: the middle of the file card header. */
  handleTop: 31,
} as const;

export const HANDLE_STYLE = { top: SIZES.handleTop } as const;

export interface Graph {
  nodes: Node[];
  edges: Edge[];
  /** Height of the laid-out content at zoom 1. */
  height: number;
}

/** Space around the content when a canvas opens at a fixed zoom. */
export const CANVAS_MARGIN = 24;

export function canvasHeight(graph: Graph, zoom: number): number {
  return Math.ceil((graph.height + CANVAS_MARGIN * 2) * zoom);
}

type NodeWithoutPosition = Omit<Node, "position">;

export interface ColumnItem {
  node: NodeWithoutPosition;
  height: number;
  /** Space above this item. Defaults to `SIZES.chipGap`. */
  gapBefore?: number;
}

export interface RowLayout {
  left: ColumnItem[];
  card: ColumnItem;
  right: ColumnItem[];
}

export function chipItem(node: NodeWithoutPosition, height: number = SIZES.chipHeight): ColumnItem {
  return { node, height };
}

export function listHeight(itemCount: number, compact = false): number {
  const item = compact ? SIZES.listCompactItem : SIZES.listItem;
  return SIZES.listHeader + itemCount * item + SIZES.listPadding;
}

function gapBefore(item: ColumnItem): number {
  return item.gapBefore ?? SIZES.chipGap;
}

function stackHeight(items: ColumnItem[]): number {
  if (items.length === 0) return 0;
  return items.reduce(
    (sum, item, index) => sum + item.height + (index === 0 ? 0 : gapBefore(item)),
    0,
  );
}

/** The same node at the same position stays the same object, so ReactFlow skips it. */
const POSITIONED = new WeakMap<NodeWithoutPosition, Map<string, Node>>();

function positioned(source: NodeWithoutPosition, x: number, y: number): Node {
  const byPosition = POSITIONED.get(source) ?? new Map<string, Node>();
  POSITIONED.set(source, byPosition);
  const key = `${x},${y}`;
  const existing = byPosition.get(key);
  if (existing) return existing;
  const node = { ...source, position: { x, y } };
  byPosition.set(key, node);
  return node;
}

function stackColumn(items: ColumnItem[], x: number, top: number): Node[] {
  let y = top;
  return items.map((item, index) => {
    if (index > 0) y += gapBefore(item);
    const node = positioned(item.node, x, y);
    y += item.height;
    return node;
  });
}

/**
 * Stack rows top to bottom. Inside a row all three columns start at the top,
 * so every row reads as one horizontal line.
 */
export function layoutRows(
  rows: RowLayout[],
  sideWidth: number = SIZES.chipWidth,
): { nodes: Node[]; height: number } {
  const cardX = sideWidth + SIZES.columnGap;
  const rightX = cardX + SIZES.cardWidth + SIZES.columnGap;
  const nodes: Node[] = [];
  let top = 0;

  for (const row of rows) {
    const height = Math.max(
      stackHeight(row.left),
      row.card.height,
      stackHeight(row.right),
    );
    nodes.push(...stackColumn(row.left, 0, top));
    nodes.push(positioned(row.card.node, cardX, top));
    nodes.push(...stackColumn(row.right, rightX, top));
    top += height + SIZES.rowGap;
  }
  return { nodes, height: Math.max(top - SIZES.rowGap, 0) };
}

export interface EdgeOptions {
  tone?: ChipTone;
  dashed?: boolean;
  width?: number;
  label?: string;
  sourceHandle?: string;
  targetHandle?: string;
  data?: Record<string, unknown>;
}

export function makeEdge(
  id: string,
  source: string,
  target: string,
  options: EdgeOptions = {},
): Edge {
  const color = EDGE_COLORS[options.tone ?? "neutral"];
  return {
    id,
    source,
    target,
    sourceHandle: options.sourceHandle,
    targetHandle: options.targetHandle,
    label: options.label,
    data: options.data,
    className: "transition-opacity duration-200",
    style: {
      stroke: color,
      strokeWidth: options.width ?? 1.5,
      strokeDasharray: options.dashed ? "5 4" : undefined,
    },
    markerEnd: { type: MarkerType.ArrowClosed, color, width: 14, height: 14 },
    labelStyle: { fontSize: 10, fontFamily: "ui-monospace, monospace" },
  };
}
