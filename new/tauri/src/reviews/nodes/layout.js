// Lays out the group rows on the canvas without measuring the DOM: every
// height is computed from the row's content. A row is one horizontal line:
// its inputs on the left, the group card in the middle, its outputs on the
// right, with an edge from each chip to the card or from the card to the chip.

import { MarkerType } from "@xyflow/svelte";
import { EDGE_COLORS } from "../change_styles.js";

export const SIZES = {
  chipWidth: 264,
  chipHeight: 40,
  chipGap: 10,
  /** Between chips of different content lines. */
  groupGap: 20,
  cardWidth: 460,
  columnGap: 130,
  rowGap: 56,
  /** Where edges meet the card, from its top: the middle of the header. */
  cardHandleTop: 30,
  chipHandleTop: 20,
};

/** The parts of a group card, from top to bottom. */
export const CARD_PARTS = {
  header: 60,
  divider: 1,
  label: 24,
  line: 30,
  bottom: 8,
  border: 2,
};

export function cardHeight(row) {
  return (
    CARD_PARTS.header +
    CARD_PARTS.divider +
    CARD_PARTS.label +
    row.lines.length * CARD_PARTS.line +
    CARD_PARTS.bottom +
    CARD_PARTS.border
  );
}

export function linkTone(link) {
  return link.is_stale ? "stale" : "neutral";
}

export function groupNodeId(groupId) {
  return `group:${groupId}`;
}

/** Chips stacked top to bottom, with a wider gap where the content line changes. */
function stackChips(links) {
  const positions = [];
  let y = 0;
  links.forEach((link, index) => {
    if (index > 0) {
      y += links[index - 1].line_key === link.line_key ? SIZES.chipGap : SIZES.groupGap;
    }
    positions.push({ link, y });
    y += SIZES.chipHeight;
  });
  return { positions, height: links.length === 0 ? 0 : y };
}

function edge(id, source, target, tone) {
  const color = EDGE_COLORS[tone];
  return {
    id,
    source,
    target,
    style: `stroke: ${color}; stroke-width: 1.5`,
    markerEnd: { type: MarkerType.ArrowClosed, color, width: 14, height: 14 },
  };
}

const FIXED = { draggable: false, selectable: false, connectable: false };

/** The nodes and edges of the rows, stacked top to bottom. */
export function buildGroupGraph(rows) {
  const nodes = [];
  const edges = [];
  const cardX = SIZES.chipWidth + SIZES.columnGap;
  const outputX = cardX + SIZES.cardWidth + SIZES.columnGap;
  let top = 0;

  for (const row of rows) {
    const cardId = groupNodeId(row.group.id);
    const inputs = stackChips(row.inputs);
    const outputs = stackChips(row.outputs);
    const height = Math.max(inputs.height, outputs.height, cardHeight(row));

    nodes.push({ id: cardId, type: "group_card", position: { x: cardX, y: top }, data: { row }, ...FIXED });
    for (const { link, y } of inputs.positions) {
      const id = `link:${row.group.id}:${link.id}`;
      nodes.push({ id, type: "link_chip", position: { x: 0, y: top + y }, data: { link }, ...FIXED });
      edges.push(edge(`${id}->card`, id, cardId, linkTone(link)));
    }
    for (const { link, y } of outputs.positions) {
      const id = `link:${row.group.id}:${link.id}`;
      nodes.push({ id, type: "link_chip", position: { x: outputX, y: top + y }, data: { link }, ...FIXED });
      edges.push(edge(`card->${id}`, cardId, id, linkTone(link)));
    }
    top += height + SIZES.rowGap;
  }

  return { nodes, edges, height: Math.max(top - SIZES.rowGap, 0) };
}
