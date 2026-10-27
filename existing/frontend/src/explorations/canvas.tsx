import { useEffect, useRef, type ReactNode } from "react";
import {
  Background,
  BackgroundVariant,
  Controls,
  ReactFlow,
  ReactFlowProvider,
  useReactFlow,
  type Edge,
  type Node,
} from "@xyflow/react";
import { useTheme } from "../theme/ThemeProvider";
import { CANVAS_MARGIN } from "./layout";
import { NODE_TYPES } from "./node_types";

export interface FocusNode {
  id: string;
  /** Changes on every request so the same node can be focused twice. */
  nonce: number;
}

function Viewport({
  fitKey,
  focusNode,
}: {
  fitKey?: string | number;
  focusNode?: FocusNode | null;
}) {
  const flow = useReactFlow();
  const previousFitKey = useRef(fitKey);

  useEffect(() => {
    if (previousFitKey.current !== fitKey) {
      previousFitKey.current = fitKey;
      void flow.fitView({ padding: 0.1, duration: 400 });
    }
  }, [fitKey, flow]);

  useEffect(() => {
    if (focusNode) {
      void flow.fitView({
        nodes: [{ id: focusNode.id }],
        padding: 0.4,
        maxZoom: 1,
        duration: 600,
      });
    }
  }, [focusNode, flow]);

  return null;
}

/**
 * A ReactFlow canvas with the settings every exploration shares.
 *
 * Embedded canvases leave the page scroll alone: hold Ctrl or Cmd to zoom
 * with the wheel. A full-page canvas sets `fullPage` to own the wheel.
 */
export function FlowCanvas({
  nodes,
  edges,
  height = 560,
  fullPage = false,
  fitKey,
  focusNode,
  zoom,
  children,
}: {
  nodes: Node[];
  edges: Edge[];
  height?: number | string;
  fullPage?: boolean;
  fitKey?: string | number;
  focusNode?: FocusNode | null;
  /** Open at this zoom, top left, instead of fitting everything in view. */
  zoom?: number;
  children?: ReactNode;
}) {
  const { resolved } = useTheme();
  return (
    <div style={{ height }} className="w-full">
      <ReactFlowProvider>
        <ReactFlow
          nodes={nodes}
          edges={edges}
          nodeTypes={NODE_TYPES}
          colorMode={resolved}
          fitView={zoom === undefined}
          fitViewOptions={{ padding: 0.1 }}
          defaultViewport={
            zoom === undefined ? undefined : { x: CANVAS_MARGIN, y: CANVAS_MARGIN, zoom }
          }
          minZoom={0.2}
          maxZoom={1.5}
          nodesDraggable={false}
          nodesConnectable={false}
          elementsSelectable={false}
          zoomOnScroll={fullPage}
          preventScrolling={fullPage}
        >
          <Background variant={BackgroundVariant.Dots} gap={18} size={1} />
          <Controls showInteractive={false} />
          <Viewport fitKey={zoom === undefined ? fitKey : undefined} focusNode={focusNode} />
          {children}
        </ReactFlow>
      </ReactFlowProvider>
    </div>
  );
}
