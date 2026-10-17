import { FlowCanvas } from "../../explorations/canvas";
import { buildChips } from "../../explorations/breadth";
import { EXAMPLE_FILES } from "../../explorations/data";
import { Link } from "react-router-dom";

const GRAPH = buildChips(EXAMPLE_FILES, false);

/** A full-page ReactFlow canvas with hardcoded data: one row per changed file. */
export default function ReactFlowBasics() {
  return (
    <div className="flex h-full flex-col">
      <header className="flex items-baseline gap-4 border-b border-base-300 px-6 py-3">
        <Link to="/explorations" className="text-sm text-primary hover:underline">
          ← Explorations
        </Link>
        <h1 className="whitespace-nowrap font-semibold">ReactFlow basics</h1>
        <p className="text-xs text-base-content/60">
          Hardcoded: {EXAMPLE_FILES.length} files, one row each. Drag to pan, wheel to zoom.
        </p>
      </header>
      <div className="min-h-0 flex-1">
        <FlowCanvas nodes={GRAPH.nodes} edges={GRAPH.edges} height="100%" fullPage />
      </div>
    </div>
  );
}
