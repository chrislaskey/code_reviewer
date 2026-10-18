import { useMemo, useState } from "react";
import { FlowActionsContext } from "../explorations/actions";
import { FlowCanvas, type FocusNode } from "../explorations/canvas";
import type { ExampleFile, OutsideMode } from "../explorations/data";
import { jumpRequest, toggled, useActions } from "../explorations/controls";
import { canvasHeight } from "../explorations/layout";
import {
  buildBlocksOnePoint,
  buildChipsOnePoint,
  buildLedgerVariant,
} from "../explorations/variants";
import { BLOCKS_LOOK, CHIPS_LOOK, LEDGER_STYLE } from "./looks";

export type ReviewView = "chips" | "ledger" | "blocks";

export const REVIEW_VIEW_OPTIONS: { value: ReviewView; label: string }[] = [
  { value: "chips", label: "Node chips" },
  { value: "ledger", label: "Ledger" },
  { value: "blocks", label: "Node blocks" },
];

const ZOOM = 0.85;
const BLOCKS_ZOOM = 0.8;

/**
 * One changed-files canvas in any of the three views. `resultKey` changes with
 * every new analysis, which puts the canvas back at the top left.
 */
export function ReviewCanvas({
  files,
  view,
  outside,
  resultKey,
}: {
  files: ExampleFile[];
  view: ReviewView;
  outside: OutsideMode;
  resultKey: string;
}) {
  const [pinned, setPinned] = useState<string | null>(null);
  const [hovered, setHovered] = useState<string | null>(null);
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const [focusNode, setFocusNode] = useState<FocusNode | null>(null);
  const focusKey = hovered ?? pinned;

  const graph = useMemo(() => {
    if (view === "ledger") {
      return buildLedgerVariant(files, {
        style: LEDGER_STYLE,
        outside,
        sort: "given",
        collapsed,
        selected: null,
      });
    }
    if (view === "blocks") {
      return buildBlocksOnePoint(files, {
        focusKey,
        outside,
        stats: "none",
        groupedByFunction: true,
        look: BLOCKS_LOOK,
      });
    }
    return buildChipsOnePoint(files, {
      focusKey,
      outside,
      stats: "none",
      compact: false,
      look: CHIPS_LOOK,
    });
  }, [files, view, outside, collapsed, focusKey]);

  const actions = useActions({
    focusFunction: setPinned,
    hoverFunction: setHovered,
    jumpToFile: (fileId) => setFocusNode(jumpRequest(`${fileId}:card`)),
    toggle: (id) => setCollapsed((current) => toggled(current, id)),
  });
  const zoom = view === "blocks" ? BLOCKS_ZOOM : ZOOM;

  return (
    <FlowActionsContext.Provider value={actions}>
      <FlowCanvas
        key={`${resultKey}:${view}`}
        nodes={graph.nodes}
        edges={graph.edges}
        height={canvasHeight(graph, zoom)}
        zoom={zoom}
        focusNode={focusNode}
      />
    </FlowActionsContext.Provider>
  );
}
