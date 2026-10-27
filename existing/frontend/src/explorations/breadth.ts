import type { Edge, Node } from "@xyflow/react";
import {
  calleeLabel,
  callerLabel,
  findFileLinks,
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
  rowHeight,
  type ChipData,
  type FileCardData,
  type GroupData,
  type ListData,
  type ListItem,
  type RowData,
} from "./nodes";

/* -- Shared pieces ---------------------------------------------------------- */

const callerId = (file: ExampleFile, index: number) => `${file.id}:in:${index}`;
const calleeId = (file: ExampleFile, index: number) => `${file.id}:out:${index}`;
const cardId = (file: ExampleFile) => `${file.id}:card`;

function cardNode(file: ExampleFile, ports: boolean): ColumnItem {
  const data: FileCardData = { file, ports };
  return {
    node: { id: cardId(file), type: "file_card", data },
    height: cardHeight(file),
  };
}

function callerSubtitle(caller: ExampleCaller): string {
  return `${shortPath(caller.file_path)}:${caller.line}`;
}

function calleeSubtitle(callee: ExampleCallee): string {
  const times = callee.call_count > 1 ? ` ×${callee.call_count}` : "";
  return `from ${callee.from}${times}`;
}

function callerChip(
  file: ExampleFile,
  caller: ExampleCaller,
  index: number,
  marked: boolean,
): ColumnItem {
  const data: ChipData = {
    side: "input",
    title: callerLabel(caller),
    subtitle: callerSubtitle(caller),
    tone: marked ? callerTone(caller) : "neutral",
    markers: marked ? callerMarkers(caller) : [],
  };
  return chipItem({ id: callerId(file, index), type: "chip", data });
}

function calleeChip(
  file: ExampleFile,
  callee: ExampleCallee,
  index: number,
  diffAware: boolean,
): ColumnItem {
  const data: ChipData = {
    side: "output",
    title: calleeLabel(callee),
    subtitle: calleeSubtitle(callee),
    tone: diffAware ? calleeTone(callee) : "neutral",
    markers: diffAware ? calleeMarkers(callee) : [],
  };
  return chipItem({ id: calleeId(file, index), type: "chip", data });
}

function listItemForCaller(caller: ExampleCaller): ListItem {
  return { title: callerLabel(caller), subtitle: caller.calls };
}

function listItemForCallee(callee: ExampleCallee): ListItem {
  return {
    title: calleeLabel(callee),
    subtitle: callee.call_count > 1 ? `×${callee.call_count}` : undefined,
  };
}

function buildRows(
  files: ExampleFile[],
  buildRow: (file: ExampleFile) => { row: RowLayout; edges: Edge[] },
): Graph {
  const built = files.map(buildRow);
  const layout = layoutRows(built.map((item) => item.row));
  return {
    nodes: layout.nodes,
    height: layout.height,
    edges: built.flatMap((item) => item.edges),
  };
}

/* -- V1: three blocks per row ----------------------------------------------- */

export function buildThreeBlocks(files: ExampleFile[]): Graph {
  return buildRows(files, (file) => {
    const inputs: ListData = {
      side: "input",
      title: `Inputs · ${file.callers.length}`,
      items: file.callers.map(listItemForCaller),
    };
    const outputs: ListData = {
      side: "output",
      title: `Outputs · ${file.callees.length}`,
      items: file.callees.map(listItemForCallee),
    };
    const inputId = `${file.id}:inputs`;
    const outputId = `${file.id}:outputs`;
    return {
      row: {
        left: [
          {
            node: { id: inputId, type: "list", data: inputs },
            height: listHeight(inputs.items.length),
          },
        ],
        card: cardNode(file, false),
        right: [
          {
            node: { id: outputId, type: "list", data: outputs },
            height: listHeight(outputs.items.length),
          },
        ],
      },
      edges: [
        makeEdge(`${inputId}->card`, inputId, cardId(file)),
        makeEdge(`card->${outputId}`, cardId(file), outputId),
      ],
    };
  });
}

/* -- V2 and V7: one chip per link, plain or with attention markers ---------- */

export function buildChips(files: ExampleFile[], marked: boolean): Graph {
  return buildRows(files, (file) => ({
    row: {
      left: file.callers.map((caller, i) => callerChip(file, caller, i, marked)),
      card: cardNode(file, false),
      right: file.callees.map((callee, i) => calleeChip(file, callee, i, false)),
    },
    edges: [
      ...file.callers.map((caller, i) =>
        makeEdge(`${callerId(file, i)}->card`, callerId(file, i), cardId(file), {
          tone: marked ? callerTone(caller) : "neutral",
          dashed: marked && caller.kind === "test",
        }),
      ),
      ...file.callees.map((_, i) =>
        makeEdge(`card->${calleeId(file, i)}`, cardId(file), calleeId(file, i)),
      ),
    ],
  }));
}

/* -- V3 and V8: function ports, plain or diff-aware ------------------------- */

export function buildPorts(files: ExampleFile[], diffAware: boolean): Graph {
  return buildRows(files, (file) => ({
    row: {
      left: file.callers.map((caller, i) => callerChip(file, caller, i, false)),
      card: cardNode(file, true),
      right: file.callees.map((callee, i) => calleeChip(file, callee, i, diffAware)),
    },
    edges: [
      ...file.callers.map((caller, i) =>
        makeEdge(`${callerId(file, i)}->card`, callerId(file, i), cardId(file), {
          targetHandle: `in:${caller.calls}`,
        }),
      ),
      ...file.callees.map((callee, i) =>
        makeEdge(`card->${calleeId(file, i)}`, cardId(file), calleeId(file, i), {
          sourceHandle: `out:${callee.from}`,
          tone: diffAware ? calleeTone(callee) : "neutral",
          dashed: diffAware && callee.diff === "removed",
        }),
      ),
    ],
  }));
}

/* -- V4: grouped by module, thicker edge for more calls --------------------- */

function edgeWidth(count: number): number {
  return Math.min(1.5 + (count - 1) * 1.5, 6);
}

function pluralCalls(count: number): string {
  return count === 1 ? "1 call" : `${count} calls`;
}

export function buildGrouped(files: ExampleFile[]): Graph {
  return buildRows(files, (file) => {
    const callerGroups = [...groupBy(file.callers, (caller) => caller.module)];
    const calleeGroups = [...groupBy(file.callees, (callee) => callee.module)];
    const edges: Edge[] = [];

    const left = callerGroups.map(([module, callers], index) => {
      const id = `${file.id}:in-group:${index}`;
      const data: GroupData = {
        side: "input",
        title: module,
        detail: [...new Set(callers.map((caller) => `${caller.name}/${caller.arity}`))].join(", "),
        count: callers.length,
      };
      for (const [target, matches] of groupBy(callers, (caller) => caller.calls)) {
        edges.push(
          makeEdge(`${id}->${target}`, id, cardId(file), {
            targetHandle: `in:${target}`,
            width: edgeWidth(matches.length),
          }),
        );
      }
      return chipItem({ id, type: "module_group", data });
    });

    const right = calleeGroups.map(([module, callees], index) => {
      const id = `${file.id}:out-group:${index}`;
      const data: GroupData = {
        side: "output",
        title: module,
        detail: [...new Set(callees.map((callee) => functionKey(callee)))].join(", "),
        count: callees.reduce((sum, callee) => sum + callee.call_count, 0),
      };
      for (const [from, matches] of groupBy(callees, (callee) => callee.from)) {
        const count = matches.reduce((sum, callee) => sum + callee.call_count, 0);
        edges.push(
          makeEdge(`${from}->${id}`, cardId(file), id, {
            sourceHandle: `out:${from}`,
            width: edgeWidth(count),
          }),
        );
      }
      return chipItem({ id, type: "module_group", data });
    });

    return { row: { left, card: cardNode(file, true), right }, edges };
  });
}

/* -- V5: ledger, one wide node per file ------------------------------------- */

export function buildLedger(files: ExampleFile[]): Graph {
  let y = 0;
  const nodes: Node[] = files.map((file) => {
    const data: RowData = { file };
    const node: Node = { id: `${file.id}:row`, type: "row", position: { x: 0, y }, data };
    y += rowHeight(file) + SIZES.rowGap / 2;
    return node;
  });
  return { nodes, edges: [], height: y };
}

/* -- V6: the changed files connected to each other -------------------------- */

export function buildFileMap(files: ExampleFile[]): Graph {
  const links = findFileLinks(files);
  const depth = new Map(files.map((file) => [file.id, 0]));
  for (let pass = 0; pass < files.length; pass++) {
    for (const link of links) {
      depth.set(
        link.to_file_id,
        Math.max(depth.get(link.to_file_id) ?? 0, (depth.get(link.from_file_id) ?? 0) + 1),
      );
    }
  }

  const columns = new Map<number, ExampleFile[]>();
  for (const file of files) {
    const column = depth.get(file.id) ?? 0;
    columns.set(column, [...(columns.get(column) ?? []), file]);
  }
  const tallest = Math.max(
    ...[...columns.values()].map((column) =>
      column.reduce((sum, file) => sum + cardHeight(file) + 40, -40),
    ),
  );

  const nodes: Node[] = [];
  for (const [column, columnFiles] of columns) {
    const height = columnFiles.reduce((sum, file) => sum + cardHeight(file) + 40, -40);
    let y = (tallest - height) / 2;
    for (const file of columnFiles) {
      const data: FileCardData = { file };
      nodes.push({
        id: cardId(file),
        type: "file_card",
        position: { x: column * (SIZES.cardWidth + 200), y },
        data,
      });
      y += cardHeight(file) + 40;
    }
  }

  const edges = links.map((link) => {
    const from = files.find((file) => file.id === link.from_file_id);
    return makeEdge(
      `${link.from_file_id}->${link.to_file_id}`,
      `${link.from_file_id}:card`,
      `${link.to_file_id}:card`,
      {
        label: `${link.functions.join(", ")} · ${pluralCalls(link.call_count)}`,
        width: edgeWidth(link.call_count),
        data: { from: from?.module },
      },
    );
  });
  return { nodes, edges, height: tallest };
}

/* -- V9: outputs split by where they live ----------------------------------- */

function originList(
  file: ExampleFile,
  origin: ExampleCallee["origin"],
  title: string,
): { item: ColumnItem; edges: Edge[] } | null {
  const callees = file.callees.filter((callee) => callee.origin === origin);
  if (callees.length === 0) return null;
  const id = `${file.id}:origin:${origin}`;
  const data: ListData = {
    side: "output",
    title: `${title} · ${callees.length}`,
    compact: true,
    items: callees.map(listItemForCallee),
  };
  const edges = [...new Set(callees.map((callee) => callee.from))].map((from) =>
    makeEdge(`${id}:${from}`, cardId(file), id, {
      sourceHandle: `out:${from}`,
      dashed: true,
      width: 1,
    }),
  );
  return {
    item: { node: { id, type: "list", data }, height: listHeight(callees.length, true) },
    edges,
  };
}

export function buildOriginLanes(files: ExampleFile[]): Graph {
  return buildRows(files, (file) => {
    const projectChips: ColumnItem[] = [];
    const edges: Edge[] = file.callers.map((caller, i) =>
      makeEdge(`${callerId(file, i)}->card`, callerId(file, i), cardId(file), {
        targetHandle: `in:${caller.calls}`,
      }),
    );
    file.callees.forEach((callee, i) => {
      if (callee.origin !== "project") return;
      projectChips.push(calleeChip(file, callee, i, false));
      edges.push(
        makeEdge(`card->${calleeId(file, i)}`, cardId(file), calleeId(file, i), {
          sourceHandle: `out:${callee.from}`,
        }),
      );
    });
    const lists = [
      originList(file, "dependency", "Dependencies"),
      originList(file, "standard_library", "Standard library"),
    ].filter((list) => list !== null);
    edges.push(...lists.flatMap((list) => list.edges));

    return {
      row: {
        left: file.callers.map((caller, i) => callerChip(file, caller, i, false)),
        card: cardNode(file, true),
        right: [...projectChips, ...lists.map((list) => list.item)],
      },
      edges,
    };
  });
}
