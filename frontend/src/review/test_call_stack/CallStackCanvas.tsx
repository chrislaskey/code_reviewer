import { useMemo } from "react";
import type { Edge, Node } from "@xyflow/react";
import type { GetTestCallStackResponse } from "../../api/types";
import { FlowActionsContext } from "../../explorations/actions";
import { FlowCanvas } from "../../explorations/canvas";
import { useActions } from "../../explorations/controls";
import { canvasHeight, makeEdge, type Graph } from "../../explorations/layout";
import { CallStackToolbar } from "./CallStackToolbar";
import { frameHeight, type CallStackFrameData } from "./flow_nodes";
import { useCallStackView, type VisibleFrame } from "./stack_view";

/*
 * The test call stack on a ReactFlow canvas: frames top to bottom, each one
 * indented under the frame that called it, with an edge from caller to call.
 */

const STACK_WIDTH = 1040;
const INDENT = 36;
const GAP = 12;
const ZOOM = 0.85;
/** Room for the zoom controls when few frames are left. */
const MIN_CANVAS_HEIGHT = 240;

function frameNodeId(index: number): string {
  return `frame:${index}`;
}

function frameIndexOf(nodeId: string): number {
  return Number(nodeId.replace("frame:", ""));
}

function buildGraph(visible: VisibleFrame[]): Graph {
  const nodes: Node[] = [];
  const edges: Edge[] = [];
  let y = 0;
  for (const item of visible) {
    const id = frameNodeId(item.index);
    const x = item.depth * INDENT;
    const data: CallStackFrameData = { ...item, width: STACK_WIDTH - x };
    nodes.push({ id, type: "call_stack_frame", position: { x, y }, data });
    y += frameHeight(data) + GAP;

    if (item.caller !== null) {
      edges.push({
        ...makeEdge(`${item.caller}->${item.index}`, frameNodeId(item.caller), id, {
          width: item.emphasis === "system" ? 1 : 1.5,
          dashed: item.emphasis === "system" || item.skippedCallers > 0,
        }),
        type: "smoothstep",
        pathOptions: { borderRadius: 8 },
      } as Edge);
    }
  }
  return { nodes, edges, height: Math.max(y - GAP, 0) };
}

export function CallStackCanvas({ stack }: { stack: GetTestCallStackResponse }) {
  const view = useCallStackView(stack.frames);
  const graph = useMemo(() => buildGraph(view.visible), [view.visible]);
  const actions = useActions({ toggle: (id) => view.toggleFrame(frameIndexOf(id)) });

  return (
    <FlowActionsContext.Provider value={actions}>
      <div className="flex min-h-0 flex-1 flex-col">
        <CallStackToolbar stack={stack} view={view} />
        <div className="min-h-0 flex-1 overflow-auto">
          <FlowCanvas
            key={`${stack.test.module}:${stack.test.name}:${view.systemCalls}:${view.unchanged}`}
            nodes={graph.nodes}
            edges={graph.edges}
            height={Math.max(canvasHeight(graph, ZOOM), MIN_CANVAS_HEIGHT)}
            zoom={ZOOM}
          />
        </div>
      </div>
    </FlowActionsContext.Provider>
  );
}
